//! Lua code generator: compiles Promela IR into Lua scripts.
//!
//! The generated Lua is loaded by the mlua runtime and called by the
//! verification engine for model-specific transition enumeration.

use crate::parser::ast::*;

/// Generated Lua code components.
#[derive(Debug, Clone)]
pub struct GeneratedLua {
    /// The full Lua source as a string.
    pub source: String,
    /// Per-proctype transition enumerator names.
    pub proctype_fn_names: Vec<String>,
}

/// Compile a Promela model into Lua source code.
pub fn generate(model: &PromelaModel) -> GeneratedLua {
    let mut g = LuaGenerator::new();
    g.emit_header();
    g.emit_state_layout(model);
    g.emit_proctypes(model);
    g.emit_trailer();
    g.finish()
}

/// Translate one Promela expression into Lua against the model's globals.
///
/// A claim's atom is evaluated against a state rather than generated as part of a transition, so
/// it needs the same name mapping the emitters use - global variables are stored under their own
/// names - but no proctype context.
pub fn expression_to_lua(model: &PromelaModel, e: &Expression) -> String {
    let mut g = LuaGenerator::new();
    for decl in &model.declarations {
        match decl {
            TopLevel::GlobalVar(v) => {
                g.global_vars.insert(v.name.clone());
            }
            TopLevel::ChanDecl { name, .. } => {
                g.global_vars.insert(name.clone());
            }
            _ => {}
        }
    }
    g.current_proctype = None;
    g.expr_to_lua(e)
}

/// Compile a Promela model into Lua source code with optimizations.
pub fn generate_with_opt(model: &PromelaModel, opt: &optimize::OptLevel) -> GeneratedLua {
    let optimized = optimize::apply_to_model(model, opt);
    let mut g = LuaGenerator::new();
    g.emit_header();
    g.emit_state_layout(&optimized);
    g.emit_proctypes(&optimized);
    g.emit_trailer();
    g.finish()
}

pub(crate) struct LuaGenerator {
    pub(crate) source: String,
    pub(crate) indent: usize,
    pub(crate) proctype_names: Vec<String>,
    pub(crate) current_proctype: Option<String>,
    /// The `provided (expr)` condition of the proctype being emitted, if it has one.
    pub(crate) current_provided: Option<Expression>,
    /// Declared type of every state field, keyed by the field name the state stores.
    ///
    /// Promela's types are fixed width and assignments wrap, so the value a variable holds is not
    /// the value the expression produced.
    pub(crate) var_types: std::collections::HashMap<String, VarType>,
    pub(crate) global_vars: std::collections::HashSet<String>,
    pub(crate) inlines: std::collections::HashMap<String, crate::parser::ast::InlineDef>,
    /// Label step numbers for goto/break/label: label_name -> step_number
    pub(crate) label_steps: std::collections::HashMap<String, usize>,
    /// Do-loop exit steps: stack of exit step numbers for nested do-loops
    pub(crate) do_exit_stack: Vec<usize>,
    /// Next available step number for labels (starts after guard body steps)
    pub(crate) next_label_step: usize,
}

mod core;
mod deps;
mod effects;
mod effects_guards;
mod expr_utils;
mod flatten;
pub mod optimize;
mod stmts;
mod stmts_atomic;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod expr_tests;

pub(crate) fn default_value(var_type: &VarType) -> String {
    match var_type {
        VarType::Bit | VarType::Bool | VarType::Byte => "0".to_string(),
        VarType::Short | VarType::Int | VarType::Unsigned(_) => "0".to_string(),
        VarType::Chan => "nil".to_string(),
        VarType::Mtype => "0".to_string(),
        VarType::Named(_) => "nil".to_string(),
    }
}
