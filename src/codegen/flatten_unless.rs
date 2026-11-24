//! Emitting the two halves of an `unless`: the escape and the steps of a handler that can pause.
//!
//! Split from the step emitter so that file stays a layout. The escape takes priority over the body:
//! while it can move the body may not, and completing it abandons the body.

use super::flatten::HandlerStepNode;
use super::*;

impl LuaGenerator {
    /// One statement of a handler that can be suspended between its statements.
    pub(crate) fn emit_handler_step(&mut self, i: usize, step_var: &str, node: &HandlerStepNode) {
        let field = self.unless_field();
        self.emit(&format!("    -- step {}: unless handler step", i));
        self.emit(&format!(
            "    guard = function(s) return s.{} >= {} and s.{} < {} and s.{} == {} and ({}) end,",
            step_var, node.from, step_var, node.to, field, node.expect, node.guard
        ));
        match node.leave {
            Some(next) => self.emit(&format!(
                "    effect = function(s) {}s.{} = {} end,",
                node.effect, field, next
            )),
            None => self.emit(&format!(
                "    effect = function(s) {}s.{} = 0; s.{} = {} end,",
                node.effect, field, step_var, node.abandon_to
            )),
        }
        let mut deps = self.deps_of(None, &[], &[field.as_str()]);
        deps.writes.insert(step_var.to_string());
        deps.reads.insert(step_var.to_string());
        deps.reads.insert(field);
        // The guard is already a rendered Lua string here, so the fields it reads were collected
        // while the handler statement was still an AST node. Without them the transition declares
        // no reads and the projection drops a field the escape depends on.
        deps.reads.extend(node.reads.iter().cloned());
        self.emit_deps(&deps);
        self.emit(&format!("    label = \"body:step{}\",", i));
    }

    /// The state fields one handler statement reads, in the generator's own field naming.
    ///
    /// This is the last point at which the statement is still an AST node; the guard and effect are
    /// Lua strings from here on. `deps_of` maps them the same way the emitters do.
    pub(crate) fn handler_step_reads(&self, st: &Stmt) -> std::collections::BTreeSet<String> {
        let map = self.map_fn_pub();
        crate::codegen::optimize::vars_read_by_stmt(st)
            .into_iter()
            .map(|v| map(&v))
            .collect()
    }
}
