//! Lua runtime bridge: connects mlua VM with the Rust verification engine.
//!
//! The runtime loads generated Lua source, exposes Rust-backed channel operations,
//! and bridges the Model trait by evaluating all transitions inside Lua.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::codegen::{self, GeneratedLua};
use crate::engine::checker::{
    CheckResult, CheckerBuilder, Model, ReductionScope, Transition, TransitionDeps,
};

/// One enabled transition, as the Lua model reports it.
#[derive(Debug, Clone)]
pub struct EnabledTransition {
    pub label: String,
    pub next_state: String,
    pub deps: TransitionDeps,
}

mod channel;
pub use channel::LuaChannel;

mod builtin_ffi;
mod chan_ffi;

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
        crate::runtime::chan_ffi::register_all(lua, channels)?;
        crate::runtime::builtin_ffi::register_all(lua)?;
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

    /// Evaluate a Lua expression against a serialized state.
    ///
    /// The state is rebuilt from the blob exactly as `enumerate_transitions` does, so the
    /// expression sees the state the caller asked about and nothing else.
    pub fn eval_bool_in_state(&self, chunk: &str, state_blob: &str) -> anyhow::Result<bool> {
        // The aliases the generated model defines for itself, from the same constant it uses: an
        // atom like `len(q) < 2` is compiled as its own chunk, and without them it called a nil
        // global and the claim was declined.
        let source = format!(
            "do local s = {{}}; {}; {}; return (({}) and true or false) end",
            crate::codegen::ALIASES,
            state_literal(state_blob),
            chunk
        );
        lua_ok(self.lua.load(source).eval::<bool>())
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
    /// Calls _spin_get_transitions(state), evaluates each (guard + effect), and returns the
    /// enabled ones with their declared dependencies. A transition that carries no metadata is
    /// reported opaque, so an unannotated emitter can never be reduced.
    pub fn enumerate_transitions(
        &self,
        state_blob: &str,
    ) -> anyhow::Result<Vec<EnabledTransition>> {
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
            let deps = crate::engine::checker::TransitionDeps {
                reads: t.get("reads").unwrap_or_default(),
                writes: t.get("writes").unwrap_or_default(),
                internal: t.get("internal").unwrap_or(false),
                // Absent metadata means unknown dependencies, never none.
                opaque: t.get("opaque").unwrap_or(true),
                owner: t.get("owner").unwrap_or_default(),
            };
            let guard_fn: mlua::Function = lua_ok(t.get("guard"))?;
            let effect_fn: mlua::Function = lua_ok(t.get("effect"))?;

            // Evaluate guard.
            //
            // Promela expressions are numeric: a guard is enabled when it is non-zero.
            // Reading the result as a Rust bool is wrong because mlua maps every
            // non-nil, non-boolean value to true, so a guard that evaluates to 0
            // (disabled) was treated as enabled. That made guarded assignments fire
            // unconditionally, e.g. a transition guarded by (a && b) taking while both
            // were zero.
            let guard_val: mlua::Value = lua_ok(guard_fn.call(state.clone()))?;
            let enabled = match guard_val {
                mlua::Value::Boolean(b) => b,
                mlua::Value::Integer(i) => i != 0,
                mlua::Value::Number(n) => n != 0.0,
                mlua::Value::Nil => false,
                _ => true,
            };
            if !enabled {
                continue;
            }

            // Clone state and apply effect
            let clone_fn: mlua::Function = lua_ok(self.lua.globals().get("_spin_clone"))?;
            let new_state: mlua::Table = lua_ok(clone_fn.call(state.clone()))?;
            lua_ok(effect_fn.call::<mlua::Value>(new_state.clone()))?;

            // Serialize new state
            let blob = lua_ok(serialize_table(&new_state))?;
            results.push(EnabledTransition {
                label,
                next_state: blob,
                deps,
            });
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
    /// Number of process instances declared by the model.
    process_count: usize,
    /// First Lua evaluation error seen during exploration.
    /// A failed guard or effect must not be mistaken for a disabled transition.
    eval_error: std::cell::RefCell<Option<String>>,
    /// True when any process contains an assert, so the failure marker is observable and no
    /// claim may reduce away the transition that writes it.
    has_asserts: bool,
    /// Names of the fields the state table actually carries, read once from the initial state.
    /// Reduction asks which of these a claim can observe, so it needs the real field names
    /// (p1_x, t1_done) rather than the Promela identifiers they came from.
    state_fields: Vec<String>,
    /// Declared `priority` per process owner, for the processes that declare one.
    ///
    /// pan runs only the highest-priority *enabled* process, so the filter belongs after
    /// enabledness is known: a blocked high-priority process does not hold up a low-priority one.
    priorities: std::collections::HashMap<String, i64>,
    /// The parsed model, kept so a claim's atom can be translated against its globals.
    atom_model: crate::parser::ast::PromelaModel,
    /// Atom text to the Lua expression that decides it, so the product search pays for the
    /// translation once rather than at every transition.
    atom_chunks: std::cell::RefCell<std::collections::HashMap<String, String>>,
}

impl LuaModel {
    /// The declared priority of the process that owns a transition; 0 when it declares none.
    fn priority_of(&self, owner: &str) -> i64 {
        self.priorities.get(owner).copied().unwrap_or(0)
    }

    /// Create from a parsed Promela model.
    pub fn from_model(model: &crate::parser::ast::PromelaModel) -> anyhow::Result<Self> {
        // A proctype that declares parameters has no encoding yet: the arguments are bound when
        // `run` instantiates it, or set to zero for an `active` one, and neither is in the state. The
        // body then reads unbound names - a channel parameter produced
        // "channel 'inp' not found in the declaration" at run time, which is a crash, and a crash is
        // neither a verdict nor a refusal.
        if let Some(p) = model.declarations.iter().find_map(|d| match d {
            crate::parser::ast::TopLevel::Proctype(p) if !p.parameters.is_empty() => Some(p),
            _ => None,
        }) {
            anyhow::bail!(
                "proctype '{}' declares parameters, which are not bound into the state yet; a process started by run or active would need its arguments there",
                p.name
            );
        }
        if let Some(why) = codegen::model_shape_refusal(model) {
            anyhow::bail!("{}", why);
        }
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
                crate::parser::ast::TopLevel::ChannelArray {
                    name,
                    size,
                    capacity,
                    ..
                } => {
                    // Register N individual channels, name_0 .. name_N-1, each with the declared
                    // capacity. This used to force capacity 0, which made every channel array a
                    // rendezvous array whatever the declaration said.
                    for i in 0..*size {
                        let chan_name = format!("{}_{}", name, i);
                        runtime.register_channel(&chan_name, *capacity as usize);
                    }
                }
                crate::parser::ast::TopLevel::Ltl(ltl) => {
                    ltl_formulas.push(ltl.clone());
                }
                _ => {}
            }
        }

        runtime.load_source(&generated)?;

        let process_count = model
            .declarations
            .iter()
            .map(|d| match d {
                crate::parser::ast::TopLevel::Proctype(p) if p.active => {
                    p.pid.map(|n| n as usize).unwrap_or(1)
                }
                crate::parser::ast::TopLevel::Init(_) => 1,
                _ => 0,
            })
            .sum();

        // The field names come from the state table itself, not from the AST: the generator
        // decides how a variable is spelled in the state (p1_x for a proctype local, x for a
        // global), and asking the AST again would be a second, divergent source of truth.
        let has_asserts = model.declarations.iter().any(|d| match d {
            crate::parser::ast::TopLevel::Proctype(p) => stmts_have_assert(&p.body),
            crate::parser::ast::TopLevel::Init(i) => stmts_have_assert(&i.body),
            _ => false,
        });

        // Owner name to priority, for the processes that declare one. The owner is the prefix the
        // generator uses, which carries the instance number when there is more than one.
        let mut priorities = std::collections::HashMap::new();
        for decl in &model.declarations {
            if let crate::parser::ast::TopLevel::Proctype(p) = decl
                && let Some(priority) = p.priority
            {
                let count = if p.active {
                    p.pid.map(|n| n as usize).unwrap_or(1)
                } else {
                    1
                };
                if count == 1 {
                    priorities.insert(p.name.clone(), priority);
                } else {
                    for instance in 0..count {
                        priorities.insert(format!("{}_{}", p.name, instance), priority);
                    }
                }
            }
        }

        let state_fields = match runtime.init_state() {
            Ok(blob) => parse_state_keys(&blob),
            Err(_) => Vec::new(),
        };

        Ok(Self {
            runtime: std::cell::RefCell::new(runtime),
            ltl_formulas,
            source: model.source.clone(),
            process_count,
            eval_error: std::cell::RefCell::new(None),
            has_asserts,
            state_fields,
            priorities,
            atom_model: model.clone(),
            atom_chunks: std::cell::RefCell::new(std::collections::HashMap::new()),
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
            Ok(results) => {
                let enabled: Vec<Transition<StateBlob>> = results
                    .into_iter()
                    .map(|t| Transition::new(t.label, StateBlob(t.next_state)).with_deps(t.deps))
                    .collect();
                // pan runs only the highest-priority enabled process. Filtering here rather than
                // in the generator is what makes a *blocked* high-priority process not hold up a
                // low-priority one.
                let top = enabled
                    .iter()
                    .map(|t| self.priority_of(&t.deps.owner))
                    .max()
                    .unwrap_or(0);
                if top > 0 {
                    return enabled
                        .into_iter()
                        .filter(|t| self.priority_of(&t.deps.owner) == top)
                        .collect();
                }
                enabled
            }
            Err(e) => {
                // Record the failure: an evaluation error is not an empty transition set.
                log::error!("Failed to enumerate transitions: {}", e);
                let mut slot = self.eval_error.borrow_mut();
                if slot.is_none() {
                    *slot = Some(format!("transition evaluation failed: {}", e));
                }
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

    fn has_claims(&self) -> bool {
        !self.ltl_formulas.is_empty()
            || self
                .atom_model
                .declarations
                .iter()
                .any(|d| matches!(d, crate::parser::ast::TopLevel::NeverClaim(_)))
    }

    fn check_violation(&self, state: &StateBlob) -> Option<String> {
        // A failed assertion is recorded in the state by the generated effect. Checked before
        // the claim gate below, because a claim disables deadlock detection (pan does the same)
        // but must not disable assertion detection.
        if let Some(what) = parse_assert_failure(&state.0) {
            return Some(format!("assertion violated: {}", what));
        }
        // With any claim present, real spin disables invalid-end-state checking - its own
        // summary prints "invalid end states - (disabled by never claim)" - because the claim
        // decides termination. Reporting a deadlock here would disagree with spin on any model
        // whose processes legitimately block awaiting a guard that never fires.
        //
        // A `never` claim counts as much as an `ltl` one. Checking only the ltl list left the
        // automaton case reporting a deadlock pan does not: measured on a process that blocks
        // at once under `never { (x == 5) }`, pan reports 0.
        if self.has_claims() {
            return None;
        }
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

    fn reduction_scopes(&self) -> Vec<ReductionScope> {
        // No fields means the initial state could not be read, so nothing is known about what
        // any claim observes. Returning no scopes leaves the search unreduced, which is correct.
        if self.state_fields.is_empty() {
            return Vec::new();
        }
        let all: std::collections::BTreeSet<String> = self.state_fields.iter().cloned().collect();
        // A model with no claim observes nothing: its only findings are deadlocks and assertion
        // failures, and an assertion failure is encoded as a disabled transition, so both are
        // deadlock findings. Persistent ample sets preserve those, and an empty visible set is
        // then the correct - not merely the cheapest - answer.
        if self.ltl_formulas.is_empty() {
            // A model with no claim observes nothing except its own assertions, which are
            // findings like any other. Leaving the marker out would let the reduction prune the
            // transition that records a failure.
            let mut visible = std::collections::BTreeSet::new();
            if self.has_asserts {
                visible.insert("_failed_assert".to_string());
            }
            return vec![ReductionScope {
                name: "model".to_string(),
                visible,
                liveness: false,
            }];
        }
        let mut out = Vec::new();
        for f in &self.ltl_formulas {
            // Liveness claims are checked by the nested-DFS path, which is never reduced: an
            // accepting cycle can consist entirely of invisible internal transitions, so
            // reducing it would hide the very violation the claim looks for.
            if !crate::engine::checker::dfs::is_safety_formula(&f.formula) {
                continue;
            }
            let name = f.name.clone().unwrap_or_else(|| "ltl".to_string());
            let visible =
                visible_fields(&f.formula, &self.state_fields).unwrap_or_else(|| all.clone());
            out.push(ReductionScope {
                name,
                visible,
                liveness: false,
            });
        }
        out
    }

    fn ltl_formulas(&self) -> &[crate::parser::ast::LtlFormula] {
        &self.ltl_formulas
    }

    fn never_claims(&self) -> Vec<(String, crate::parser::ast::NeverClaim)> {
        self.atom_model
            .declarations
            .iter()
            .filter_map(|d| match d {
                crate::parser::ast::TopLevel::NeverClaim(c) => Some(c.clone()),
                _ => None,
            })
            .enumerate()
            .map(|(i, c)| (format!("never_{}", i), c))
            .collect()
    }

    fn state_to_string(&self, state: &Self::State) -> Option<String> {
        Some(state.0.clone())
    }

    fn process_count(&self) -> usize {
        self.process_count
    }

    /// Decide a claim's atom by evaluating it against the state.
    ///
    /// `atomic_props` answers one field at a time, which covers a bare identifier. A comparison
    /// like `a == b` is an expression over several fields, so it is parsed and translated once
    /// per atom and evaluated against the state blob the caller names. The translation is cached
    /// because the product search asks for the same handful of atoms at every transition.
    fn eval_atom(&self, state: &Self::State, atom: &str) -> Option<bool> {
        const PREDEFINED: &[&str] = &["true", "false", "nil"];
        if PREDEFINED.contains(&atom) {
            return None; // answered by evaluate_atomic_props
        }
        let chunk = {
            let mut cache = self.atom_chunks.borrow_mut();
            match cache.get(atom) {
                Some(c) => c.clone(),
                None => {
                    let expr = crate::parser::parse_expression(atom).ok()?;
                    let lua = crate::codegen::expression_to_lua(&self.atom_model, &expr);
                    cache.insert(atom.to_string(), lua.clone());
                    lua
                }
            }
        };
        let runtime = self.runtime.borrow();
        match runtime.eval_bool_in_state(&chunk, &state.0) {
            Ok(v) => Some(v),
            Err(e) => {
                // An atom that cannot be evaluated is not false; record it so the caller reports
                // the claim as unverified rather than deciding it.
                let mut slot = self.eval_error.borrow_mut();
                if slot.is_none() {
                    *slot = Some(format!("atom '{}' could not be evaluated: {}", atom, e));
                }
                None
            }
        }
    }

    fn atomic_props(&self, state: &Self::State) -> Option<std::collections::HashMap<String, bool>> {
        // Parse the serialized state blob. Format: {"_done_P":false,"x":0,"flag":1,...}
        let mut props = std::collections::HashMap::new();
        let inner = state.0.trim_start_matches('{').trim_end_matches('}');
        for entry in inner.split(',') {
            let entry = entry.trim();
            if let Some(colon_pos) = entry.find(':') {
                let key = entry[..colon_pos].trim().trim_matches('"');
                let value = entry[colon_pos + 1..].trim();
                // Numeric variables: 0 is false, non-zero is true.
                // Boolean variables arrive as true/false/nil.
                let bool_val = value != "0" && value != "false" && value != "nil";
                props.insert(key.to_string(), bool_val);
            }
        }
        Some(props)
    }
}

// ─── Reduction scopes ───────────────────────────────────────────

/// A formula token that names no model variable.
const LTL_KEYWORDS: &[&str] = &[
    "true",
    "false",
    "nil",
    "U",
    "V",
    "W",
    "X",
    "skip",
    "always",
    "eventually",
    "until",
];

/// Field names carried by a serialized state blob.
fn parse_state_keys(blob: &str) -> Vec<String> {
    let inner = blob.trim_start_matches('{').trim_end_matches('}');
    let mut out = Vec::new();
    for entry in inner.split(',') {
        let entry = entry.trim();
        if let Some(colon) = entry.find(':') {
            let key = entry[..colon].trim().trim_matches('"');
            if !key.is_empty() {
                out.push(key.to_string());
            }
        }
    }
    out
}

/// Every identifier a formula mentions.
fn formula_identifiers(formula: &str) -> Vec<String> {
    let bytes = formula.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            while i < bytes.len() && {
                let c = bytes[i] as char;
                c.is_ascii_alphanumeric() || c == '_'
            } {
                i += 1;
            }
            out.push(formula[start..i].to_string());
        } else {
            i += 1;
        }
    }
    out
}

/// The state fields whose change can alter a safety claim's truth value.
///
/// Returns None when an identifier cannot be resolved to any field the state carries, so the
/// caller can fall back to treating everything as visible. Guessing in that situation - treating
/// an unresolved atom as visible-not-relevant - is how a reduced search loses a violation.
fn visible_fields(
    formula: &str,
    state_fields: &[String],
) -> Option<std::collections::BTreeSet<String>> {
    let mut out = std::collections::BTreeSet::new();
    for ident in formula_identifiers(formula) {
        if LTL_KEYWORDS.contains(&ident.as_str()) {
            continue;
        }
        // A proctype local x is stored as p1_x, so a formula naming x observes every process's
        // copy. Over-approximating here costs reduction; under-approximating would be unsound.
        let suffix = format!("_{}", ident);
        let mut matched = false;
        for field in state_fields {
            if *field == ident || field.ends_with(&suffix) {
                out.insert(field.clone());
                matched = true;
            }
        }
        if !matched {
            return None;
        }
    }
    if out.is_empty() { None } else { Some(out) }
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

/// Wrap an evaluation failure so callers can downcast it instead of matching text.
fn typed_eval_failure(detail: String) -> anyhow::Error {
    anyhow::Error::new(crate::engine::checker::VerifyError::EvalFailed { detail })
}

/// Parse Promela, generate Lua, create a checker, and run once.
pub fn verify(source: &str) -> anyhow::Result<CheckResult> {
    let model = LuaModel::from_source(source)?;
    let checker = CheckerBuilder::new().model(model).build();
    // `check()`, not `check_dfs()`: the latter is the bare unreduced traversal and skips
    // everything the configured entry point adds - reduction, and the claims this engine does not
    // check. Calling it directly is how `verify` came to disagree with `verify_with_config` on
    // the same source for the second time.
    let result = checker.check();
    if let Some(detail) = checker.model().take_error() {
        return Err(typed_eval_failure(detail));
    }
    Ok(result)
}

impl LuaModel {
    /// Get LTL formulas from the model.
    pub fn ltl_formulas(&self) -> &[crate::parser::ast::LtlFormula] {
        &self.ltl_formulas
    }

    /// Recreate model from stored source (for property checking).
    /// Take the first evaluation error seen during exploration, if any.
    ///
    /// A guard or effect that failed to evaluate produces an empty transition list, which
    /// is indistinguishable from a disabled transition. Callers must treat a recorded
    /// error as a verification failure rather than as "no transitions were enabled".
    pub fn take_error(&self) -> Option<String> {
        self.eval_error.borrow_mut().take()
    }

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

/// The assertion recorded in a state, if the state carries one.
///
/// The generated effect writes `_failed_assert` only when the condition is false, so the key
/// being present at all is the failure.
fn parse_assert_failure(blob: &str) -> Option<String> {
    let pos = blob.find("_failed_assert")?;
    // The key itself is quoted in the blob: "_failed_assert":"assert(x)". Splitting on the
    // first colon and skipping the opening quote is the reliable route; trimming quotes directly
    // took the key's closing quote for the value's opening one and returned a colon.
    let rest = &blob[pos + "_failed_assert".len()..];
    let after_colon = rest.split_once(':').map(|(_, v)| v).unwrap_or(rest);
    let value = after_colon.trim_start_matches([' ', '"']);
    let end = value.find(['"', ',']).unwrap_or(value.len());
    Some(value[..end].to_string())
}

fn stmt_has_assert(s: &crate::parser::ast::Stmt) -> bool {
    use crate::parser::ast::Stmt;
    match s {
        Stmt::Assert(_, _) => true,
        Stmt::If(gs) | Stmt::Do(gs) => gs.iter().any(|g| stmts_have_assert(&g.body)),
        Stmt::Atomic(b, _) | Stmt::DStep(b, _) => stmts_have_assert(b),
        Stmt::Unless { body, handler, .. } => stmts_have_assert(body) || stmts_have_assert(handler),
        Stmt::For {
            init, update, body, ..
        } => stmt_has_assert(init) || stmt_has_assert(update) || stmts_have_assert(body),
        _ => false,
    }
}

fn stmts_have_assert(stmts: &[crate::parser::ast::Stmt]) -> bool {
    stmts.iter().any(stmt_has_assert)
}
