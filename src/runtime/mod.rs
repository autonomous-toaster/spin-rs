//! Lua runtime bridge: connects mlua VM with the Rust verification engine.
//!
//! The runtime loads generated Lua source, exposes Rust-backed channel operations,
//! and bridges the Model trait by evaluating all transitions inside Lua.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::codegen::{self, GeneratedLua};
use crate::engine::checker::{CheckResult, CheckerBuilder, Model, Transition};

mod channel;
pub use channel::LuaChannel;

mod serialize;
pub(crate) use serialize::{serialize_table, state_literal};

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
        Self::register_chan_send(lua, Arc::clone(&channels))?;
        Self::register_chan_recv(lua, Arc::clone(&channels))?;
        Self::register_chan_peek(lua, Arc::clone(&channels))?;
        Self::register_chan_len(lua, Arc::clone(&channels))?;
        Self::register_chan_full(lua, Arc::clone(&channels))?;
        Self::register_chan_empty(lua, Arc::clone(&channels))?;
        Self::register_assert(lua)?;
        Self::register_printf(lua)?;
        Self::register_state_hash(lua)?;
        Self::register_remote_ref(lua)?;
        Self::register_fairness_track(lua)?;
        Self::register_c_code(lua)?;
        Self::register_stubborn_dep(lua)?;
        Ok(())
    }

    fn register_chan_send(
        lua: &mlua::Lua,
        channels: Arc<Mutex<HashMap<String, LuaChannel>>>,
    ) -> mlua::Result<()> {
        let f = lua.create_function(move |lua, args: mlua::MultiValue| {
            let mut iter = args.into_iter();
            // First arg is state table
            let state: mlua::Table = match iter.next() {
                Some(mlua::Value::Table(s)) => s,
                _ => {
                    return Err(mlua::Error::runtime(
                        "chan_send: expected state table as first arg",
                    ));
                }
            };
            // Second arg is channel name
            let name = match iter.next() {
                Some(mlua::Value::String(s)) => s.to_string_lossy().to_string(),
                _ => {
                    return Err(mlua::Error::runtime(
                        "chan_send: expected string channel name as second arg",
                    ));
                }
            };
            // Remaining args are message values
            let mut parts = Vec::new();
            for arg in iter {
                match arg {
                    mlua::Value::Integer(i) => parts.push(i),
                    mlua::Value::Number(n) => parts.push(n as i64),
                    _ => return Err(mlua::Error::runtime("chan_send: expected integer values")),
                }
            }
            let mut chans = channels.lock().unwrap();
            if let Some(chan) = chans.get_mut(&name) {
                if !chan.send(parts.clone()) {
                    return Err(mlua::Error::runtime("channel full"));
                }
                // Update state table to reflect channel contents
                let chan_state: mlua::Table = lua.create_table()?;
                for (i, val) in chan.messages.iter().enumerate() {
                    if let Some(&first) = val.first() {
                        chan_state.set(i + 1, first)?;
                    }
                }
                state.set(name.clone(), chan_state)?;
                Ok(())
            } else {
                Err(mlua::Error::runtime(format!(
                    "channel '{}' not found (possible out-of-bounds array access)",
                    name
                )))
            }
        })?;
        lua.globals().set("_spin_chan_send", f)?;
        Ok(())
    }

    fn register_chan_recv(
        lua: &mlua::Lua,
        channels: Arc<Mutex<HashMap<String, LuaChannel>>>,
    ) -> mlua::Result<()> {
        let f = lua.create_function(move |lua, args: mlua::MultiValue| {
            let mut iter = args.into_iter();
            // First arg is state table
            let state: mlua::Table = match iter.next() {
                Some(mlua::Value::Table(s)) => s,
                _ => {
                    return Err(mlua::Error::runtime(
                        "chan_recv: expected state table as first arg",
                    ));
                }
            };
            // Second arg is channel name
            let name = match iter.next() {
                Some(mlua::Value::String(s)) => s.to_string_lossy().to_string(),
                _ => {
                    return Err(mlua::Error::runtime(
                        "chan_recv: expected string channel name as second arg",
                    ));
                }
            };
            let mut chans = channels.lock().unwrap();
            if let Some(chan) = chans.get_mut(&name) {
                match chan.recv() {
                    Some(msg) => {
                        // Update state table to reflect channel contents
                        let chan_state: mlua::Table = lua.create_table()?;
                        for (i, val) in chan.messages.iter().enumerate() {
                            if let Some(&first) = val.first() {
                                chan_state.set(i + 1, first)?;
                            }
                        }
                        state.set(name.clone(), chan_state)?;
                        Ok(mlua::Value::Integer(msg.first().copied().unwrap_or(0)))
                    }
                    None => Err(mlua::Error::runtime("channel empty")),
                }
            } else {
                Err(mlua::Error::runtime(format!(
                    "channel '{}' not found (possible out-of-bounds array access)",
                    name
                )))
            }
        })?;
        lua.globals().set("_spin_chan_recv", f)?;
        Ok(())
    }

    fn register_chan_peek(
        lua: &mlua::Lua,
        _channels: Arc<Mutex<HashMap<String, LuaChannel>>>,
    ) -> mlua::Result<()> {
        let f = lua.create_function(move |_lua, (state, name): (mlua::Table, String)| {
            // First try to get from state table (which has the current channel state)
            let chan_state: mlua::Table = match state.get(name.as_str()) {
                Ok(t) => t,
                Err(_) => return Err(mlua::Error::runtime("channel not found in state")),
            };
            // Get first element (index 1)
            match chan_state.get(1i32) {
                Ok(mlua::Value::Integer(i)) => Ok(mlua::Value::Integer(i)),
                Ok(mlua::Value::Nil) => Err(mlua::Error::runtime("channel empty")),
                Err(e) => Err(mlua::Error::runtime(format!("channel peek error: {}", e))),
                _ => Err(mlua::Error::runtime("channel peek: unexpected value type")),
            }
        })?;
        lua.globals().set("_spin_chan_peek", f)?;
        Ok(())
    }

    fn register_chan_len(
        lua: &mlua::Lua,
        channels: Arc<Mutex<HashMap<String, LuaChannel>>>,
    ) -> mlua::Result<()> {
        let f = lua.create_function(move |_lua, name: String| {
            let chans = channels.lock().unwrap();
            match chans.get(&name) {
                Some(chan) => Ok(chan.len() as i64),
                None => Err(mlua::Error::runtime(format!(
                    "channel '{}' not found",
                    name
                ))),
            }
        })?;
        lua.globals().set("_spin_chan_len", f)?;
        Ok(())
    }

    fn register_chan_full(
        lua: &mlua::Lua,
        channels: Arc<Mutex<HashMap<String, LuaChannel>>>,
    ) -> mlua::Result<()> {
        let f = lua.create_function(move |_lua, name: String| {
            let chans = channels.lock().unwrap();
            match chans.get(&name) {
                Some(chan) => Ok(chan.is_full()),
                None => Err(mlua::Error::runtime(format!(
                    "channel '{}' not found",
                    name
                ))),
            }
        })?;
        lua.globals().set("_spin_chan_full", f)?;
        Ok(())
    }

    fn register_chan_empty(
        lua: &mlua::Lua,
        channels: Arc<Mutex<HashMap<String, LuaChannel>>>,
    ) -> mlua::Result<()> {
        let f = lua.create_function(move |_lua, name: String| {
            let chans = channels.lock().unwrap();
            match chans.get(&name) {
                Some(chan) => Ok(chan.is_empty()),
                None => Err(mlua::Error::runtime(format!(
                    "channel '{}' not found",
                    name
                ))),
            }
        })?;
        lua.globals().set("_spin_chan_empty", f)?;
        Ok(())
    }

    fn register_assert(lua: &mlua::Lua) -> mlua::Result<()> {
        let f = lua.create_function(|_lua, (cond, msg): (bool, String)| {
            if !cond {
                Err(mlua::Error::runtime(msg))
            } else {
                Ok(())
            }
        })?;
        lua.globals().set("_spin_assert", f)?;
        Ok(())
    }

    fn register_printf(lua: &mlua::Lua) -> mlua::Result<()> {
        let f = lua.create_function(|_lua, args: mlua::MultiValue| {
            let parts: Vec<String> = args.iter().map(|v| format!("{:?}", v)).collect();
            eprintln!("{}", parts.join(" "));
            Ok(())
        })?;
        lua.globals().set("_spin_printf", f)?;
        Ok(())
    }

    fn register_state_hash(lua: &mlua::Lua) -> mlua::Result<()> {
        let f = lua.create_function(|_lua, state: mlua::Table| serialize_table(&state))?;
        lua.globals().set("_spin_state_hash", f)?;
        Ok(())
    }

    fn register_remote_ref(lua: &mlua::Lua) -> mlua::Result<()> {
        let f = lua.create_function(|_lua, (pid, var): (i64, String)| {
            Ok(format!("<remote {}:{}>", pid, var))
        })?;
        lua.globals().set("_spin_remote_ref", f)?;
        Ok(())
    }

    fn register_fairness_track(lua: &mlua::Lua) -> mlua::Result<()> {
        let f = lua.create_function(|_lua, (_label, _enabled): (String, bool)| Ok(()))?;
        lua.globals().set("_spin_fairness_track", f)?;
        Ok(())
    }

    fn register_c_code(lua: &mlua::Lua) -> mlua::Result<()> {
        let f = lua.create_function(|lua, code: String| {
            lua.load(&code)
                .exec()
                .map_err(|e| mlua::Error::runtime(format!("c_code: {}", e)))
        })?;
        lua.globals().set("_spin_c_code", f)?;
        Ok(())
    }

    fn register_stubborn_dep(lua: &mlua::Lua) -> mlua::Result<()> {
        let f = lua.create_function(|_lua, (_t1, _t2): (String, String)| Ok(false))?;
        lua.globals().set("_spin_stubborn_dep", f)?;
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

// ─── Model Trait Implementation ─────────────────────────────────

/// A serialized state used as the state type in the Model trait.
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct StateBlob(pub String);

/// A model backed by a Lua runtime executing generated Promela code.
pub struct LuaModel {
    runtime: std::cell::RefCell<LuaRuntime>,
    ltl_formulas: Vec<crate::parser::ast::LtlFormula>,
    source: Option<String>,
}

impl LuaModel {
    /// Create from a parsed Promela model.
    pub fn from_model(model: &crate::parser::ast::PromelaModel) -> anyhow::Result<Self> {
        let generated = codegen::generate(model);
        let mut runtime = LuaRuntime::new()?;
        let mut ltl_formulas = Vec::new();

        for decl in &model.declarations {
            match decl {
                crate::parser::ast::TopLevel::ChanDecl { name, capacity, .. } => {
                    runtime.register_channel(name, *capacity as usize);
                }
                // Also handle channels parsed as GlobalVar with Chan type (fallback)
                crate::parser::ast::TopLevel::GlobalVar(v)
                    if v.var_type == crate::parser::ast::VarType::Chan =>
                {
                    // Extract capacity from init expression if available
                    let capacity = 0; // Default to rendezvous
                    runtime.register_channel(&v.name, capacity);
                }
                crate::parser::ast::TopLevel::ChannelArray { name, size, .. } => {
                    // Register N individual rendezvous channels: name_0, name_1, ...
                    for i in 0..*size {
                        let chan_name = format!("{}_{}", name, i);
                        runtime.register_channel(&chan_name, 0); // All rendezvous (capacity 0)
                    }
                }
                crate::parser::ast::TopLevel::Ltl(ltl) => {
                    ltl_formulas.push(ltl.clone());
                }
                _ => {}
            }
        }

        runtime.load_source(&generated)?;

        Ok(Self {
            runtime: std::cell::RefCell::new(runtime),
            ltl_formulas,
            source: model.source.clone(),
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
        let runtime = self.runtime.borrow();
        match runtime.init_state() {
            Ok(blob) => vec![StateBlob(blob)],
            Err(e) => {
                log::error!("Failed to create initial state: {}", e);
                vec![]
            }
        }
    }

    fn transitions(&self, state: &StateBlob) -> Vec<Transition<StateBlob>> {
        let runtime = self.runtime.borrow();
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

    fn check_violation(&self, state: &StateBlob) -> Option<String> {
        // Deadlock detection: no transitions but active processes remain
        let trans = self.transitions(state);
        if !trans.is_empty() {
            return None;
        }
        if state.0.len() < 15 {
            return None;
        }
        // Parse _done_<name> flags from state blob to count still-running processes
        // State blob format: {"key":val,"key":val,...} or {key:val,key:val,...}
        let (total_done, running) = parse_done_flags(&state.0);
        // Only flag deadlock when at least one process is still running (_done == false)
        // If all processes are done (running == 0), it's normal termination, not deadlock
        if running > 0 {
            return Some("deadlock: some processes blocked".to_string());
        }
        // Fallback: if no _done_ flags found, check nr_pr >= 2 as before
        if total_done == 0 {
            let nr_pr = if let Some(pos) = state.0.find("_nr_pr") {
                let after_key = &state.0[pos + 7..]; // skip "_nr_pr"
                let after_colon = after_key.trim_start_matches([':', '"']);
                let num_str: String = after_colon
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                num_str.parse::<i64>().unwrap_or(0)
            } else {
                0
            };
            if nr_pr >= 2 {
                return Some("deadlock: some processes blocked".to_string());
            }
        }
        None
    }

    fn ltl_formulas(&self) -> &[crate::parser::ast::LtlFormula] {
        &self.ltl_formulas
    }

    fn state_to_string(&self, state: &Self::State) -> Option<String> {
        Some(state.0.clone())
    }
}

// ─── Parsing helpers ────────────────────────────────────────────

/// Parse `_done_<name>` flags from a state blob to determine how many
/// processes are still running (_done == false) vs done (_done == true).
/// Returns (total_done_flags_found, running_count).
fn parse_done_flags(blob: &str) -> (usize, usize) {
    // State blob format: {key:val,key:val,...}
    // Look for _done_<name>:true or _done_<name>:false patterns
    let mut total = 0usize;
    let mut running = 0usize;
    let mut pos = 0;
    while let Some(start) = blob[pos..].find("_done_") {
        let actual_pos = pos + start;
        // Find the value after this key: look for ':' then check next token
        if let Some(colon_pos) = blob[actual_pos..].find(':') {
            let val_start = actual_pos + colon_pos + 1;
            // Value is true, false, or a quoted version
            let rest = &blob[val_start..];
            if rest.starts_with("true") || rest.starts_with("\"true\"") {
                total += 1;
                // Process is done
            } else if rest.starts_with("false") || rest.starts_with("\"false\"") {
                total += 1;
                running += 1;
            }
        }
        pos = actual_pos + 7; // advance past "_done_"
        if pos >= blob.len() {
            break;
        }
    }
    (total, running)
}

// ─── Convenience ────────────────────────────────────────────────

/// Parse Promela, generate Lua, create a checker, and run once.
pub fn verify(source: &str) -> anyhow::Result<CheckResult> {
    let model = LuaModel::from_source(source)?;
    let checker = CheckerBuilder::new().model(model).build();
    Ok(checker.check_dfs())
}

impl LuaModel {
    /// Get LTL formulas from the model.
    pub fn ltl_formulas(&self) -> &[crate::parser::ast::LtlFormula] {
        &self.ltl_formulas
    }

    /// Recreate model from stored source (for property checking).
    pub fn recreate(&self) -> anyhow::Result<Self> {
        if let Some(ref source) = self.source {
            Self::from_source(source)
        } else {
            anyhow::bail!("no source stored")
        }
    }
}

#[cfg(test)]
mod tests;
