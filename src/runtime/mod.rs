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

mod layout;
mod serialize;

// The layout helpers are used throughout this module; re-exported so the call sites stay short.
use layout::{
    is_control_field, parse_done_flags, parse_state_keys, stmts_have_assert, stmts_have_goto,
    visible_fields,
};
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
-- A state table is flat: every value is a scalar. The recursive form below calls type() on
-- every field of every copy, and _spin_clone runs once per enabled transition per state, which
-- makes it one of the two hottest functions in the search. The shallow path is taken when no
-- value is a table, and the recursive form is kept for models that do nest.
function _spin_clone(t)
    if type(t) ~= 'table' then return t end
    local copy = {}
    local nested = false
    for k, v in pairs(t) do
        if type(v) == 'table' then nested = true
        else copy[k] = v end
    end
    if nested then
        for k, v in pairs(t) do copy[k] = _spin_clone(v) end
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

    /// Rebuild a state table from its serialized form without compiling a Lua chunk.
    ///
    /// The blob is `{key:value,...}` with flat scalar values, so each entry is a plain field
    /// assignment. Compiling `do local s={...} return s end` per state visit was ~100x pan's
    /// per-state cost on its own: one Lua parse and compile for every state explored.
    ///
    /// Entries whose value is a nested table or is not a literal fall back to the compiling path,
    /// so this is a fast path and not a second implementation of the parser.
    fn table_from_blob(&self, blob: &str) -> anyhow::Result<mlua::Table> {
        let table = self.lua.create_table()?;
        let inner = blob
            .strip_prefix('{')
            .and_then(|s| s.strip_suffix('}'))
            .unwrap_or(blob);
        if inner.trim().is_empty() {
            return Ok(table);
        }
        for entry in inner.split(',') {
            let Some((key, value)) = entry.split_once(':') else {
                return self.table_from_blob_slow(blob);
            };
            let key = key.trim().trim_matches('"');
            let value = value.trim();
            // A nested table or an unexpected shape is not handled here.
            if value.starts_with('{') || value.contains(['{', '}']) {
                return self.table_from_blob_slow(blob);
            }
            let v = match value {
                "true" => mlua::Value::Boolean(true),
                "false" => mlua::Value::Boolean(false),
                "nil" => mlua::Value::Nil,
                _ => {
                    let unquoted = value.trim_matches('"');
                    match unquoted.parse::<i64>() {
                        Ok(i) => mlua::Value::Integer(i),
                        Err(_) => match unquoted.parse::<f64>() {
                            Ok(f) => mlua::Value::Number(f),
                            // A string value: keep it exactly as serialized.
                            Err(_) => mlua::Value::String(self.lua.create_string(unquoted)?),
                        },
                    }
                }
            };
            table.set(key, v)?;
        }
        Ok(table)
    }

    /// The compiling path, used when a state carries a nested table.
    fn table_from_blob_slow(&self, blob: &str) -> anyhow::Result<mlua::Table> {
        lua_ok(
            self.lua
                .load(format!(
                    "do local s={{}}; {} return s end",
                    state_literal(blob)
                ))
                .eval(),
        )
    }

    /// Every field any transition in the table reads, disabled transitions included.
    ///
    /// `_spin_get_transitions` returns the whole table and the guard filter is applied in Rust, so
    /// reading `reads` here sees transitions no state enables yet. Expanding a state instead is not
    /// equivalent: on the initial state of a two-process model only the first process's step is
    /// present, because the other is blocked - and the blocked process's guard is exactly the
    /// dependency being looked for.
    pub fn all_read_fields(&self, state_blob: &str) -> anyhow::Result<Vec<String>> {
        let state = self.table_from_blob(state_blob)?;
        let get_fn: mlua::Function = lua_ok(self.lua.globals().get("_spin_get_transitions"))?;
        let trans_table: mlua::Table = lua_ok(get_fn.call(state.clone()))?;
        let mut out = std::collections::BTreeSet::new();
        for pair_result in trans_table.pairs::<mlua::Value, mlua::Table>() {
            let (_idx, t) = lua_ok(pair_result)?;
            let reads: Vec<String> = t.get("reads").unwrap_or_default();
            out.extend(reads);
        }
        Ok(out.into_iter().collect())
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
        // Reconstruct the state table without compiling a chunk. This is the per-state hot path:
        // one Lua compile per state visit was the largest single cost of the search.
        let state = self.table_from_blob(state_blob)?;

        // One crossing per state: the expansion evaluates every guard, clones the state and
        // applies the effect inside Lua, then hands back only the enabled successors.
        //
        // Doing the same work from Rust costs one crossing per transition, and on a wide model
        // that is dozens per state. Measured at 422 fields: 991us of a 1031us state expansion was
        // the crossings; the string parse and projection the earlier work suspected were 40us of
        // it between them. The guard semantics (a guard is enabled when non-zero, nil is
        // disabled, a boolean is taken as itself) are preserved inside _spin_expand.
        let expand_fn: mlua::Function = lua_ok(self.lua.globals().get("_spin_expand"))?;
        let expanded: mlua::Table = lua_ok(expand_fn.call(state))?;

        let mut results = Vec::new();
        for pair_result in expanded.pairs::<mlua::Value, mlua::Table>() {
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
            let next_state: mlua::Table = lua_ok(t.get("next_state"))?;
            results.push(EnabledTransition {
                label,
                next_state: lua_ok(serialize_table(&next_state))?,
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
    /// The state fields any `assert` expression reads.
    ///
    /// A finding's inputs must be visible to reduction: a writer of a field an assert reads is not
    /// independent of the assert. `_failed_assert` records the failure; these are what decides it.
    assert_field_reads: Vec<String>,
    /// Does the model declare a channel? Channel contents are not in the state vector, so a
    /// projection would lose the queue and merge states that differ only in occupancy.
    has_channels: bool,
    /// Fields that a *guard* reads, and which therefore decide which transitions are enabled.
    ///
    /// A claim that does not name `fB` still cannot merge two states that differ in a field some
    /// guard branches on: the enabled transition set would be different, so the merged state
    /// silently loses every transition behind the false branch. Measured on
    ///   bool fA, fB;
    ///   active proctype A() { do :: (1) -> fA = 1 od }
    ///   active proctype B() { do :: (fA) -> fB = 1; break od }
    ///   ltl p { [] ( !fB ) }
    /// where projecting onto the claim's own variables dropped `fA` and lost the violation.
    guard_fields: Vec<String>,
    /// Does any body jump through a label?
    ///
    /// A jump step declares only the step counter as a dependency, which makes it look independent of
    /// every other process - and the reduction then drops the interleavings that a labelled loop needs.
    has_goto: bool,
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
    /// Does every name this atom mentions exist in the state or in the model?
    ///
    /// An atom compiled against a bare state reads a missing field as nil, and nil compares false - so
    /// a claim about a *local* variable, which the state has no copy of, was refuted rather than left
    /// undecided. pan answers 0 for such a claim, because it resolves the name to something else
    /// entirely; neither answer is a verdict this engine can stand behind, and the honest outcome is
    /// to decline it by naming the atom.
    fn atom_names_resolve(&self, expr: &crate::parser::ast::Expression, state: &StateBlob) -> bool {
        use crate::parser::ast::{Expression as E, TopLevel};
        let mut ok = true;
        crate::codegen::walk_expr(expr, &mut |e| {
            if !ok {
                return;
            }
            let E::Ident(name) = e else { return };
            if self.atom_model.mtype_names.contains_key(name) {
                return;
            }
            if crate::engine::checker::dfs::extract_var_from_state(&state.0, name).is_some() {
                return;
            }
            let declared = self.atom_model.declarations.iter().any(|d| match d {
                TopLevel::GlobalVar(v) => v.name == *name,
                TopLevel::ChanDecl { name: n, .. } => n == name,
                TopLevel::ChannelArray { name: n, .. } => n == name,
                _ => false,
            });
            if !declared {
                ok = false;
            }
        });
        ok
    }
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
        // A proctype's parameters are bound into the state under the instance prefix, as pan binds
        // them: an `active` process starts with every parameter zero, which is measurable -
        // `active proctype P(byte n) { assert(n == 0) }` is pan 0 and `assert(n == 1)` is pan 1. A
        // process started by `run` needs its arguments bound at the call, and `run` still refuses by
        // name, so nothing here is guessed.
        // An inline is a macro: expand every call into the body it names before anything is laid out,
        // so the step machine never sees a call. This has to happen here rather than per statement,
        // because a call inside a guard, an atomic region or another inline would otherwise reach an
        // arm the layout does not have - which is how it used to become a call to a nil Lua global.
        let expanded = codegen::inline::expand(model).map_err(|e| anyhow::anyhow!("{e}"))?;
        let model = &expanded;
        // The shape check runs on the *expanded* model. It ran on the original until an inline body
        // carrying `timeout` slipped past it and the model was decided by a guard that is always
        // false; a check that inspects a different model than the emitter is the same defect it is
        // meant to catch.
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
        // Fields the asserts read. Computed once: reduction needs them on every traversal and the
        // answer cannot change during a search.
        let assert_field_reads: Vec<String> =
            layout::model_assert_reads(model).into_iter().collect();
        // Does the model declare a channel?
        //
        // Channel contents are *not* part of the serialized state: they live in the Lua runtime's
        // shared registry, outside the state vector. So state identity already under-describes a
        // model with channels, and projecting on top of that makes two states that differ only in
        // queue occupancy indistinguishable. Measured: the rendezvous model
        // `chan c = [0] of { bit }; ... ltl p0 { [] ( !got ) }` went from 6 states and 1 error to
        // 1 state and 0 errors. A projection of a state that does not contain everything is not a
        // reduction.
        let has_channels = model.declarations.iter().any(|d| {
            matches!(
                d,
                crate::parser::ast::TopLevel::ChanDecl { .. }
                    | crate::parser::ast::TopLevel::ChannelArray { .. }
                    | crate::parser::ast::TopLevel::Typedef { .. }
            )
        });
        let has_goto = model.declarations.iter().any(|d| match d {
            crate::parser::ast::TopLevel::Proctype(p) => stmts_have_goto(&p.body),
            crate::parser::ast::TopLevel::Init(i) => stmts_have_goto(&i.body),
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

        // Every field any transition's guard reads.
        //
        // Collected by expanding the initial state once. The runtime's transition table is
        // constructed by the generator with declared `reads` sets, so one expansion sees every
        // transition that exists - they are all present before any guard is evaluated.
        //
        // Cheap: it is one enumeration at construction, against a search that expands thousands.
        let guard_fields: Vec<String> = match runtime.init_state() {
            Ok(init) => runtime.all_read_fields(&init).unwrap_or_default(),
            Err(_) => Vec::new(),
        };

        Ok(Self {
            runtime: std::cell::RefCell::new(runtime),
            ltl_formulas,
            source: model.source.clone(),
            process_count,
            eval_error: std::cell::RefCell::new(None),
            has_asserts,
            assert_field_reads,
            has_channels,
            guard_fields,
            has_goto,
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
                // A state that cannot be built is not an empty state space. Swallowing the error
                // made the search explore nothing and report success - on a model whose initialisers
                // reference `_pid`, which is bound per instance and not in the layout, the answer was
                // "no errors" where pan refutes. Recorded so the caller reports it instead.
                *self.eval_error.borrow_mut() =
                    Some(format!("the initial state could not be built: {e}"));
                Vec::new()
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

    fn project(
        &self,
        state: &StateBlob,
        visible: &std::collections::BTreeSet<String>,
    ) -> StateBlob {
        // Three cases where the state is returned unchanged, and each one is a correctness
        // boundary rather than a caution:
        //
        // 1. An empty visible set means the caller does not know what the claim observes.
        // 2. A model with channels keeps its queue contents outside the serialized state, so the
        //    state vector already under-describes the system before any projection. Merging two
        //    states that differ only in queue occupancy loses transitions outright - measured as
        //    a rendezvous model going from 6 states and 1 error to 1 state and 0 errors.
        // 3. A model whose initial state could not be read has no field list to project on.
        if visible.is_empty() || self.has_channels || self.state_fields.is_empty() {
            return state.clone();
        }
        StateBlob(crate::runtime::serialize::project_blob(&state.0, visible))
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
        // A model with goto or labels is not reduced.
        //
        // Measured on spin's own ex_3c.pml: reduced, 77 states and no error; unreduced, 858 states and
        // the assertion pan reports (1). A jump step touches only the step counter, so the ample-set
        // computation sees it as independent of everything - and a body that loops back through a
        // label then loses the interleaving that puts two processes in the same critical section. The
        // reduction is a performance device; a model it makes appear correct is worse than a slow one.
        if self.has_goto {
            return Vec::new();
        }
        // A model with channels is not reduced at all.
        //
        // The same reason the projection is disabled for these models, and it is the same root
        // cause: the state vector does not carry channel contents. Reduction reasons about
        // independence from declared reads and writes, and an unmodelled queue makes two
        // genuinely dependent steps look independent.
        if self.has_channels {
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
            // With no claim there is nothing to project onto: every variable can enable a guard,
            // so dropping one stops the search progressing. The projection's premise is that a
            // claim cannot observe a field; with no claim that premise is false for every field.
            //
            // Measured: projecting onto the control fields alone made
            //   byte n = 0; active proctype P() { do :: (n < 10) -> n = n + 1 od }
            // with max_depth 3 explore one state and never reach the depth limit it exists to
            // report. The set is returned whole and projection is the identity for such a model.
            let mut identity: std::collections::BTreeSet<String> =
                self.state_fields.iter().cloned().collect();
            // _failed_assert is not in the initial state's field list - it appears only once an
            // assertion has failed - so building identity from state_fields alone makes the failed
            // state project onto the same identity as the state before it, and the search
            // deduplicates the violation away.
            //
            // Measured on a claim-free model:
            //   byte x = 0;
            //   active proctype A() { do :: x = 1; x = 2 od }
            //   active proctype B() { assert(x != 1) }
            // pan refutes in 11 states; with _failed_assert absent from identity the reduced search
            // accepted in 10.
            if self.has_asserts {
                identity.insert("_failed_assert".to_string());
            }
            return vec![ReductionScope {
                name: "model".to_string(),
                // Reduction still sees only what a finding can read. Widening this disabled the
                // ample-set computation on every model, which is the defect this field split
                // exists to prevent.
                visible: {
                    let mut v = std::collections::BTreeSet::new();
                    if self.has_asserts {
                        v.insert("_failed_assert".to_string());
                        // What the asserts READ, not only where a failure is recorded. A writer of
                        // a field an assert reads is not independent of that assert.
                        v.extend(self.assert_field_reads.iter().cloned());
                    }
                    v
                },
                identity,
                liveness: false,
            }];
        }
        let mut out = Vec::new();
        for f in &self.ltl_formulas {
            // A liveness claim is never REDUCED: an accepting cycle can consist entirely of
            // invisible internal transitions, so an ample set would hide the very violation the
            // claim looks for. But it is still projected for state IDENTITY.
            //
            // Those are different operations and the distinction matters here. Reduction removes
            // transitions, so an invisible cycle can disappear. Projection only merges states the
            // claim cannot tell apart; the cycle still exists in the projected graph, and the
            // fields that decide which transitions are enabled are retained below. Skipping the
            // claim entirely made a liveness-only model pay the full 2^k: measured 10232 states on
            // a ten-task chain against 102 with the projection, same verdict.
            let is_safety = crate::engine::checker::dfs::is_safety_formula(&f.formula);
            let name = f.name.clone().unwrap_or_else(|| "ltl".to_string());
            let visible =
                visible_fields(&f.formula, &self.state_fields).unwrap_or_else(|| all.clone());
            // State identity is a *different* question from reduction visibility, and the two
            // sets must not be merged. See ReductionScope::identity.
            let mut identity = visible.clone();
            // Control-flow fields are visible to every claim, whether or not the claim names them.
            //
            // A claim observes the variables it mentions, but state *identity* is not the same set:
            // the program counters and step counters decide which transitions are enabled, so two
            // states that agree on `x` and disagree on `_pc_P` are different states that a
            // projection must not merge. Measured with only the claim-visible set, the model
            //   byte x = 0; active proctype P() { do :: x < 5 -> x++ od unless (x == 3) }
            //   ltl p0 { [] ( x <= 2 ) }
            // collapsed to a single state and reported zero errors where pan reports one.
            for field in &self.state_fields {
                if is_control_field(field) {
                    identity.insert(field.clone());
                }
            }
            // A field any guard branches on is visible to every claim, whether or not the claim
            // names it. The claim decides which *violations* matter; the guards decide which
            // *transitions* exist. Merging two states that differ in a guard field loses the
            // transitions behind the branch that is false in one of them.
            //
            // This is the same trap the cycle proviso documents for reduction, in a different
            // place: reduction can keep an invisible field because it reasons per transition, but
            // a state projection drops the field for the whole run.
            identity.extend(self.guard_fields.iter().cloned());
            // The assertion marker is visible to every claim, whether or not the claim names it.
            //
            // This is not an over-approximation for the reduction's benefit: `check_violation`
            // reads it before the claim gate and reports the failure whatever the claim says, so a
            // field that can change a finding must not be droppable from state identity. Task 2.8
            // asserts it, and leaving it out would let a hidden bit hide an assertion failure -
            // the same trap the `has_goto` guard above documents for reduction.
            if self.has_asserts {
                identity.insert("_failed_assert".to_string());
            }
            out.push(ReductionScope {
                name,
                visible,
                identity,
                // A liveness claim is never reduced, whatever its identity set is. The checker
                // already refuses to reduce when this is true, which is what keeps the accepting
                // cycle intact while the projection still merges indistinguishable states.
                liveness: !is_safety,
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
                    let mut expr = crate::parser::parse_expression(atom).ok()?;
                    // An atom is a string parsed here, so it never went through the AST walk that
                    // rewrites mtype names into their values in bodies. Without it, a comparison
                    // against an mtype constant read a field that does not exist and answered false,
                    // which refuted a liveness claim pan accepts.
                    if !self.atom_model.mtype_names.is_empty() {
                        crate::parser::ast::PromelaModel::resolve_mtype_in_expr(
                            &mut expr,
                            &self.atom_model.mtype_names,
                        );
                    }
                    // A name the state does not carry is *unresolved*, not false: reading a missing
                    // field as false refuted claims about local variables, which the state has no
                    // copy of. The empty string is the cache's way of remembering that answer, so
                    // later calls for the same atom do not re-ask.
                    if !self.atom_names_resolve(&expr, state) {
                        cache.insert(atom.to_string(), String::new());
                        return None;
                    }
                    let lua = crate::codegen::expression_to_lua(&self.atom_model, &expr);
                    cache.insert(atom.to_string(), lua.clone());
                    lua
                }
            }
        };
        if chunk.is_empty() {
            return None;
        }
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
mod projection_tests;
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
