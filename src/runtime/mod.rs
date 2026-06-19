//! Lua runtime bridge: connects mlua VM with the Rust verification engine.
//!
//! The runtime loads generated Lua source, exposes Rust-backed channel operations,
//! and bridges the Model trait by evaluating all transitions inside Lua.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::codegen::{self, GeneratedLua};
use crate::engine::checker::{CheckResult, CheckerBuilder, Model, Transition};

/// A bounded channel with message queue, backing Promela channel operations.
#[derive(Debug, Clone)]
pub struct LuaChannel {
    capacity: usize,
    messages: Vec<Vec<i64>>,
}

impl LuaChannel {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            messages: Vec::new(),
        }
    }

    pub fn send(&mut self, msg: Vec<i64>) -> Result<(), ()> {
        if self.capacity > 0 && self.messages.len() >= self.capacity {
            return Err(());
        }
        self.messages.push(msg);
        Ok(())
    }

    pub fn recv(&mut self) -> Option<Vec<i64>> {
        if self.messages.is_empty() {
            None
        } else {
            Some(self.messages.remove(0))
        }
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }
    pub fn is_full(&self) -> bool {
        self.capacity > 0 && self.messages.len() >= self.capacity
    }
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

/// Wrapper to make Lua operations return anyhow::Result by boxing the non-Send error.
fn lua_ok<T>(r: mlua::Result<T>) -> anyhow::Result<T> {
    r.map_err(|e| anyhow::anyhow!("Lua error: {}", e))
}

/// The Lua runtime instance, wrapping an mlua engine.
pub struct LuaRuntime {
    lua: mlua::Lua,
    /// Registry of named channels shared across the model.
    channels: Arc<Mutex<HashMap<String, LuaChannel>>>,
}

impl LuaRuntime {
    /// Create a new Lua runtime, bootstrapping the environment.
    pub fn new() -> anyhow::Result<Self> {
        let lua = mlua::Lua::new();
        let channels: Arc<Mutex<HashMap<String, LuaChannel>>> =
            Arc::new(Mutex::new(HashMap::new()));

        Self::register_functions(&lua, Arc::clone(&channels))
            .map_err(|e| anyhow::anyhow!("Lua init: {}", e))?;

        Ok(Self { lua, channels })
    }

    fn register_functions(
        lua: &mlua::Lua,
        channels: Arc<Mutex<HashMap<String, LuaChannel>>>,
    ) -> mlua::Result<()> {
        // Channel send
        let ch = Arc::clone(&channels);
        let f = lua.create_function(move |_lua, args: mlua::MultiValue| {
            let mut iter = args.into_iter();
            let name = match iter.next() {
                Some(mlua::Value::String(s)) => s.to_string_lossy().to_string(),
                _ => {
                    return Err(mlua::Error::runtime(
                        "chan_send: expected string channel name",
                    ))
                }
            };
            let mut parts = Vec::new();
            for arg in iter {
                match arg {
                    mlua::Value::Integer(i) => parts.push(i),
                    mlua::Value::Number(n) => parts.push(n as i64),
                    _ => return Err(mlua::Error::runtime("chan_send: expected integer values")),
                }
            }
            let mut chans = ch.lock().unwrap();
            if let Some(chan) = chans.get_mut(&name) {
                chan.send(parts)
                    .map_err(|_| mlua::Error::runtime("channel full"))?;
                Ok(())
            } else {
                Err(mlua::Error::runtime(format!(
                    "channel '{}' not found",
                    name
                )))
            }
        })?;
        lua.globals().set("_spin_chan_send", f)?;

        // Channel receive
        let ch = Arc::clone(&channels);
        let f = lua.create_function(move |_lua, args: mlua::MultiValue| {
            let name = match args.into_iter().next() {
                Some(mlua::Value::String(s)) => s.to_string_lossy().to_string(),
                _ => {
                    return Err(mlua::Error::runtime(
                        "chan_recv: expected string channel name",
                    ))
                }
            };
            let mut chans = ch.lock().unwrap();
            if let Some(chan) = chans.get_mut(&name) {
                match chan.recv() {
                    Some(msg) => Ok(mlua::Value::Integer(msg.first().copied().unwrap_or(0))),
                    None => Err(mlua::Error::runtime("channel empty")),
                }
            } else {
                Err(mlua::Error::runtime(format!(
                    "channel '{}' not found",
                    name
                )))
            }
        })?;
        lua.globals().set("_spin_chan_recv", f)?;

        // Channel length
        let ch = Arc::clone(&channels);
        let f = lua.create_function(move |_lua, name: String| {
            let chans = ch.lock().unwrap();
            match chans.get(&name) {
                Some(chan) => Ok(chan.len() as i64),
                None => Err(mlua::Error::runtime(format!(
                    "channel '{}' not found",
                    name
                ))),
            }
        })?;
        lua.globals().set("_spin_chan_len", f)?;

        // Channel full check
        let ch = Arc::clone(&channels);
        let f = lua.create_function(move |_lua, name: String| {
            let chans = ch.lock().unwrap();
            match chans.get(&name) {
                Some(chan) => Ok(chan.is_full()),
                None => Err(mlua::Error::runtime(format!(
                    "channel '{}' not found",
                    name
                ))),
            }
        })?;
        lua.globals().set("_spin_chan_full", f)?;

        // Channel empty check
        let ch = Arc::clone(&channels);
        let f = lua.create_function(move |_lua, name: String| {
            let chans = ch.lock().unwrap();
            match chans.get(&name) {
                Some(chan) => Ok(chan.is_empty()),
                None => Err(mlua::Error::runtime(format!(
                    "channel '{}' not found",
                    name
                ))),
            }
        })?;
        lua.globals().set("_spin_chan_empty", f)?;

        // Assertion
        let f = lua.create_function(|_lua, (cond, msg): (bool, String)| {
            if !cond {
                Err(mlua::Error::runtime(msg))
            } else {
                Ok(())
            }
        })?;
        lua.globals().set("_spin_assert", f)?;

        // Printf (to stderr)
        let f = lua.create_function(|_lua, args: mlua::MultiValue| {
            let parts: Vec<String> = args.iter().map(|v| format!("{:?}", v)).collect();
            eprintln!("{}", parts.join(" "));
            Ok(())
        })?;
        lua.globals().set("_spin_printf", f)?;

        // State hashing (serialize table to deterministic string)
        let f = lua.create_function(|_lua, state: mlua::Table| serialize_table(&state))?;
        lua.globals().set("_spin_state_hash", f)?;

        Ok(())
    }

    /// Load generated Lua source into the runtime.
    pub fn load_source(&mut self, generated: &GeneratedLua) -> anyhow::Result<()> {
        let preamble = r#"
function _spin_clone(t)
    if type(t) ~= 'table' then return t end
    local copy = {}
    for k, v in pairs(t) do
        copy[k] = _spin_clone(v)
    end
    return copy
end
"#;
        let full_source = format!("{}{}", preamble, generated.source);
        lua_ok(self.lua.load(&full_source).exec())
    }

    /// Load generated Lua from a raw string.
    pub fn load_source_str(&mut self, source: &str) -> anyhow::Result<()> {
        let full_source = format!(
            "function _spin_clone(t) if type(t)~='table' then return t end local c={{}} for k,v in pairs(t) do c[k]=_spin_clone(v) end return c end\n{}",
            source
        );
        lua_ok(self.lua.load(&full_source).exec())
    }

    /// Get or create a channel by name.
    pub fn register_channel(&mut self, name: &str, capacity: usize) {
        self.channels
            .lock()
            .unwrap()
            .entry(name.to_string())
            .or_insert_with(|| LuaChannel::new(capacity));
    }

    /// Create initial state table and return serialized form.
    pub fn init_state(&self) -> anyhow::Result<String> {
        let init_fn: mlua::Function = lua_ok(self.lua.globals().get("_spin_init_state"))?;
        let state: mlua::Table = lua_ok(init_fn.call(()))?;
        lua_ok(serialize_table(&state)).map_err(|e| anyhow::anyhow!("state serialization: {}", e))
    }

    /// Enumerate all transitions from a serialized state.
    ///
    /// Calls `_spin_get_transitions(state)`, evaluates each (guard + effect),
    /// returns (label, serialized_next_state) for enabled transitions.
    pub fn enumerate_transitions(&self, state_blob: &str) -> anyhow::Result<Vec<(String, String)>> {
        // Reconstruct Lua state table from serialized blob
        let state: mlua::Table = lua_ok(
            self.lua
                .load(format!(
                    "do local s={{}}; {} return s end",
                    state_literal(state_blob)
                ))
                .eval(),
        )?;

        let get_fn: mlua::Function = lua_ok(self.lua.globals().get("_spin_get_transitions"))?;
        let trans_table: mlua::Table = lua_ok(get_fn.call(state.clone()))?;

        let mut results = Vec::new();

        let pair_iter = trans_table.pairs::<mlua::Value, mlua::Table>();
        for pair_result in pair_iter {
            let (_idx, t) = lua_ok(pair_result)?;
            let label: String = t.get("label").unwrap_or_default();
            let guard_fn: mlua::Function = lua_ok(t.get("guard"))?;
            let effect_fn: mlua::Function = lua_ok(t.get("effect"))?;

            // Evaluate guard
            let enabled: bool = lua_ok(guard_fn.call(state.clone()))?;
            if !enabled {
                continue;
            }

            // Clone state and apply effect
            let clone_fn: mlua::Function = lua_ok(self.lua.globals().get("_spin_clone"))?;
            let new_state: mlua::Table = lua_ok(clone_fn.call(state.clone()))?;
            lua_ok(effect_fn.call::<mlua::Value>(new_state.clone()))?;

            // Serialize new state
            let blob = lua_ok(serialize_table(&new_state))?;
            results.push((label, blob));
        }

        Ok(results)
    }
}

// ─── Serialization ──────────────────────────────────────────────

/// Serialize a Lua table to a deterministic string for hashing and equality.
fn serialize_table(table: &mlua::Table) -> mlua::Result<String> {
    let mut entries: Vec<(String, String)> = Vec::new();
    for pair in table.pairs::<mlua::Value, mlua::Value>() {
        let (key, value) = pair?;
        entries.push((val_to_string(&key), val_to_string(&value)));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut out = String::from("{");
    for (i, (k, v)) in entries.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(k);
        out.push(':');
        out.push_str(v);
    }
    out.push('}');
    Ok(out)
}

/// Build a Lua literal expression to reconstruct a serialized state table.
fn state_literal(blob: &str) -> String {
    let inner = blob.trim_start_matches('{').trim_end_matches('}');
    if inner.is_empty() {
        return String::new();
    }
    inner
        .split(',')
        .filter(|s| !s.is_empty())
        .filter_map(|entry| {
            let idx = entry.find(':')?;
            let (k, v) = entry.split_at(idx);
            let v = &v[1..];
            Some(format!("s[{}] = {}", k, v))
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// Format a Lua value as a compact string for state serialization.
fn val_to_string(value: &mlua::Value) -> String {
    match value {
        mlua::Value::Nil => "nil".to_string(),
        mlua::Value::Boolean(b) => b.to_string(),
        mlua::Value::Integer(i) => i.to_string(),
        mlua::Value::Number(n) => {
            if *n == (*n as i64) as f64 {
                format!("{}", *n as i64)
            } else {
                format!("{}", n)
            }
        }
        mlua::Value::String(s) => {
            if let Ok(s) = s.to_str() {
                format!("\"{}\"", s)
            } else {
                "nil".to_string()
            }
        }
        mlua::Value::Table(t) => {
            let mut parts: Vec<String> = Vec::new();
            for (k, v) in t.clone().pairs::<mlua::Value, mlua::Value>().flatten() {
                parts.push(format!("{}:{}", val_to_string(&k), val_to_string(&v)));
            }
            parts.sort();
            format!("{{{}}}", parts.join(","))
        }
        _ => "nil".to_string(),
    }
}

// ─── Model Trait Implementation ─────────────────────────────────

/// A serialized state used as the state type in the Model trait.
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct StateBlob(pub String);

/// A model backed by a Lua runtime executing generated Promela code.
pub struct LuaModel {
    runtime: Arc<Mutex<LuaRuntime>>,
}

impl LuaModel {
    /// Create from a parsed Promela model.
    pub fn from_model(model: &crate::parser::ast::PromelaModel) -> anyhow::Result<Self> {
        let generated = codegen::generate(model);
        let mut runtime = LuaRuntime::new()?;

        for decl in &model.declarations {
            if let crate::parser::ast::TopLevel::ChanDecl { name, capacity, .. } = decl {
                runtime.register_channel(name, *capacity as usize);
            }
        }

        runtime.load_source(&generated)?;

        Ok(Self {
            runtime: Arc::new(Mutex::new(runtime)),
        })
    }

    /// Create from raw Promela source.
    pub fn from_source(source: &str) -> anyhow::Result<Self> {
        let model = crate::parser::parse(source)?;
        Self::from_model(&model)
    }
}

impl Model for LuaModel {
    type State = StateBlob;

    fn init_states(&self) -> Vec<StateBlob> {
        let runtime = self.runtime.lock().unwrap();
        match runtime.init_state() {
            Ok(blob) => vec![StateBlob(blob)],
            Err(e) => {
                log::error!("Failed to create initial state: {}", e);
                vec![]
            }
        }
    }

    fn transitions(&self, state: &StateBlob) -> Vec<Transition<StateBlob>> {
        let runtime = self.runtime.lock().unwrap();
        match runtime.enumerate_transitions(&state.0) {
            Ok(results) => results
                .into_iter()
                .map(|(label, blob)| Transition {
                    label,
                    next: StateBlob(blob),
                })
                .collect(),
            Err(e) => {
                log::error!("Failed to enumerate transitions: {}", e);
                vec![]
            }
        }
    }

    fn hash(&self, state: &StateBlob) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = fxhash::FxHasher::default();
        state.0.hash(&mut hasher);
        hasher.finish()
    }
}

// ─── Convenience ────────────────────────────────────────────────

/// Parse Promela, generate Lua, create a checker, and run once.
pub fn verify(source: &str) -> anyhow::Result<CheckResult> {
    let model = LuaModel::from_source(source)?;
    let checker = CheckerBuilder::new().model(model).build();
    Ok(checker.check_dfs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lua_runtime_init() {
        let rt = LuaRuntime::new().unwrap();
        // No init_state call since _spin_init_state requires generated code
        assert!(rt
            .lua
            .globals()
            .get::<mlua::Function>("_spin_printf")
            .is_ok());
        assert!(rt
            .lua
            .globals()
            .get::<mlua::Function>("_spin_assert")
            .is_ok());
        assert!(rt
            .lua
            .globals()
            .get::<mlua::Function>("_spin_chan_send")
            .is_ok());
        assert!(rt
            .lua
            .globals()
            .get::<mlua::Function>("_spin_chan_recv")
            .is_ok());
    }

    #[test]
    fn test_channel_send_recv() {
        let mut rt = LuaRuntime::new().unwrap();
        rt.register_channel("q", 10);
        let chans = rt.channels.lock().unwrap();
        let chan = chans.get("q").unwrap();
        let mut c = chan.clone();
        assert!(c.send(vec![42]).is_ok());
        assert_eq!(c.recv(), Some(vec![42]));
        assert!(c.is_empty());
    }

    #[test]
    fn test_verify_simple() {
        let source = "active proctype P() { byte x; x = 1; }";
        let result = verify(source).unwrap();
        assert_eq!(result.errors, 0);
        assert!(result.states_explored > 0);
    }
}
