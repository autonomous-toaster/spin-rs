//! Per-transition dependency metadata for partial order reduction.
//!
//! Reduction needs to know, for every transition, which state fields it reads and writes, and
//! whether it touches anything a property could observe. That is known exactly at generation
//! time, so it is emitted alongside the transition rather than reconstructed later from the
//! label - parsing labels is how the previous attempt at this module classified every transition
//! as writing the variable "do".
//!
//! Field names are the names in the Lua state table, not the Promela names, because those are
//! what actually collide: a proctype-local `x` in `p1` is stored as `p1_x` and cannot collide
//! with `x` in `p2`.

use std::collections::BTreeSet;

use crate::parser::ast::{Expression, RecvTarget, Stmt};

use super::LuaGenerator;
use super::optimize::{vars_in_expr, vars_read_by_stmt, vars_written_by_stmt};

/// State fields that belong to the engine's control machinery rather than to the model.
pub(crate) const INTERNAL_PREFIXES: &[&str] = &["_step_", "_done_", "_pc_", "_progress_"];

/// Exact set of state fields a transition touches.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Deps {
    pub reads: BTreeSet<String>,
    pub writes: BTreeSet<String>,
}

impl Deps {
    pub(crate) fn read_expr(&mut self, e: &Expression, map: &dyn Fn(&str) -> String) {
        self.reads.extend(vars_in_expr(e).iter().map(|v| map(v)));
    }

    fn read_stmt(&mut self, s: &Stmt, map: &dyn Fn(&str) -> String) {
        self.reads
            .extend(vars_read_by_stmt(s).iter().map(|v| map(v)));
        if let Stmt::Recv {
            target: RecvTarget::Eval(e),
            ..
        } = s
        {
            self.read_expr(e, map);
        }
    }

    fn write_stmt(&mut self, s: &Stmt, map: &dyn Fn(&str) -> String) {
        self.writes
            .extend(vars_written_by_stmt(s).iter().map(|v| map(v)));
    }

    pub(crate) fn write_field(&mut self, field: impl Into<String>) -> &mut Self {
        self.writes.insert(field.into());
        self
    }

    /// Fold another declaration into this one.
    ///
    /// A transition whose guard is the negation of several other guards reads everything any of
    /// them reads, so its declaration is their union.
    pub(crate) fn merge(&mut self, other: &Deps) {
        self.reads.extend(other.reads.iter().cloned());
        self.writes.extend(other.writes.iter().cloned());
    }
}

/// Is this transition invisible to every property and independent of every other process?
///
/// True when it touches only the owner's own bookkeeping or its own locals. Such a transition
/// cannot change any variable a claim can name, and no other process's transition can touch the
/// same fields. Transitions with the same owner are never treated as independent of each other:
/// several guards can be enabled at one step, and those are alternatives, not concurrent actions.
pub(crate) fn is_internal(owner: &str, writes: &BTreeSet<String>) -> bool {
    let local_prefix = format!("{}_", owner);
    writes
        .iter()
        .all(|w| INTERNAL_PREFIXES.iter().any(|p| w.starts_with(p)) || w.starts_with(&local_prefix))
}

/// Render the metadata fields for a Lua transition table entry.
///
/// `opaque` means the emitter could not state this transition's dependencies precisely. An opaque
/// transition conflicts with every other transition and is always visible, so it disables
/// reduction around itself. That is the safe direction: an empty read/write set would instead
/// claim independence from everything, which is exactly the unsound case.
pub(crate) fn lua_fields(owner: &str, deps: &Deps, opaque: bool) -> String {
    let list = |set: &BTreeSet<String>| {
        set.iter()
            .map(|v| format!("\"{}\"", v))
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        "owner = \"{}\", reads = {{{}}}, writes = {{{}}}, internal = {}, opaque = {},",
        owner,
        list(&deps.reads),
        list(&deps.writes),
        !opaque && is_internal(owner, &deps.writes),
        opaque
    )
}

impl LuaGenerator {
    /// Map a Promela variable name to the state field that stores it.
    pub(crate) fn state_field(&self, name: &str) -> String {
        if name == "_pid" {
            "_pid".to_string()
        } else if self.global_vars.contains(name) {
            name.to_string()
        } else if let Some(ref pname) = self.current_proctype {
            format!("{}_{}", pname, name)
        } else {
            name.to_string()
        }
    }

    fn map_fn(&self) -> impl Fn(&str) -> String + '_ {
        move |n: &str| self.state_field(n)
    }

    /// Same mapping, boxed so callers outside an expression context can hold it.
    pub(crate) fn map_fn_pub(&self) -> impl Fn(&str) -> String + '_ {
        move |n: &str| self.state_field(n)
    }

    /// Dependencies of a transition that evaluates `guard` and executes `body`, plus any
    /// engine field the emitter writes itself.
    pub(crate) fn deps_of(
        &self,
        guard: Option<&Expression>,
        body: &[&Stmt],
        engine_writes: &[&str],
    ) -> Deps {
        let map = self.map_fn();
        let mut d = Deps::default();
        if let Some(g) = guard {
            d.read_expr(g, &map);
        }
        for s in body {
            d.read_stmt(s, &map);
            d.write_stmt(s, &map);
        }
        for w in engine_writes {
            d.write_field(*w);
        }
        d
    }

    /// Dependencies of a transition that only reads guard-like expressions and writes engine
    /// fields - the guard-selection step, which executes no body statement.
    pub(crate) fn deps_selection(&self, readings: &[&Expression], engine_writes: &[&str]) -> Deps {
        let map = self.map_fn();
        let mut d = Deps::default();
        for e in readings {
            d.read_expr(e, &map);
        }
        for w in engine_writes {
            d.write_field(*w);
        }
        d
    }

    /// Emit the metadata line for a precisely analysed transition.
    pub(crate) fn emit_deps(&mut self, deps: &Deps) {
        let owner = self.owner_name();
        let text = lua_fields(&owner, deps, false);
        self.emit(&format!("    {}", text));
    }

    /// Emit metadata for a transition whose dependencies this emitter cannot state precisely.
    /// Marking it opaque keeps it dependent with everything, which loses reduction and stays sound.
    pub(crate) fn emit_opaque_deps(&mut self) {
        let owner = self.owner_name();
        let text = lua_fields(&owner, &Deps::default(), true);
        self.emit(&format!("    {}", text));
    }

    /// Open a transition table entry whose dependencies this emitter cannot state precisely.
    ///
    /// Used for channel operations, whose effect runs inside the Rust FFI and whose interference
    /// with other transitions is not a simple variable access. Opaque is the sound choice: it
    /// keeps such a transition dependent with everything, so it simply blocks reduction around
    /// itself.
    pub(crate) fn open_opaque_transition(&mut self) {
        self.emit("    table.insert(transitions, {");
        let owner = self.owner_name();
        let text = lua_fields(&owner, &Deps::default(), true);
        self.emit(&format!("        {}", text));
    }

    /// Public form for emitters building a Lua literal by hand.
    pub(crate) fn owner_for_deps(&self) -> String {
        self.owner_name()
    }

    /// The effect fragment that records a failed assertion.
    ///
    /// A failed assert used to disable its own transition, which made it indistinguishable from a
    /// guard that is simply not satisfied. That is only observable when the whole model stops, so
    /// a failed assertion in a process that blocks while another keeps running was invisible:
    ///
    /// ```text
    /// active proctype A() { do :: (1) -> skip od }
    /// active proctype B() { do :: (1) -> assert(false); break od }
    /// ```
    ///
    /// reported zero errors where pan reports one. The condition is now evaluated and recorded in
    /// the state, which the checker reads as a violation - which is only sound now that a body is
    /// sequenced, so an assert cannot run before the writes that precede it.
    ///
    /// Promela truthiness is "not zero", which Lua does not share - 0 is truthy there - so the test
    /// is written out rather than left to an if-not.
    pub(crate) fn assert_failure_effect(&self, cond_lua: &str, label: &str) -> String {
        // Keep the label out of the generated Lua string literal's way.
        let safe: String = label
            .chars()
            .filter(|c| *c != '\u{27}' && !c.is_control())
            .collect();
        format!(
            "local _av = ({}); if _av == false or _av == nil or _av == 0 then s._failed_assert = '{}' end",
            cond_lua, safe
        )
    }

    fn owner_name(&self) -> String {
        self.current_proctype
            .clone()
            .unwrap_or_else(|| "init".to_string())
    }
}
