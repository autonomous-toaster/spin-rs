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
        self.emit_deps(&deps);
        self.emit(&format!("    label = \"body:step{}\",", i));
    }
}
