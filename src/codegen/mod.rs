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

pub(crate) struct LuaGenerator {
    pub(crate) source: String,
    pub(crate) indent: usize,
    pub(crate) proctype_names: Vec<String>,
    pub(crate) current_proctype: Option<String>,
    pub(crate) global_vars: std::collections::HashSet<String>,
}

mod core;
mod effects;
mod expr_utils;
mod stmts;

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
