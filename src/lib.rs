//! # spin-rs
//!
//! A Rust-native Promela model checker with Lua runtime.
//!
//! This library provides a complete model checking pipeline:
//! - **Parser**: Promela source → AST
//! - **Codegen**: AST → Lua source
//! - **Runtime**: Lua VM (mlua) with Rust-backed channel primitives
//! - **Engine**: DFS/BFS state exploration with POR support
//! - **Property**: LTL verification with nested DFS
//! - **Trail**: Error trail generation and replay
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use spin_rs::{verify, CheckResult};
//!
//! let promela = r#"
//!     active proctype P() {
//!         byte x = 0;
//!         x = 1;
//!         assert(x == 1);
//!     }
//! "#;
//!
//! let result: CheckResult = verify(promela).unwrap();
//! assert_eq!(result.errors, 0);
//! ```
//!
//! ## Advanced Usage
//!
//! ```rust,no_run
//! use spin_rs::{CheckerBuilder, StorageMode, SearchMode, LuaModel};
//!
//! let promela = std::fs::read_to_string("model.pml").unwrap();
//! let model = LuaModel::from_source(&promela).unwrap();
//!
//! let checker = CheckerBuilder::new()
//!     .model(model)
//!     .max_states(1_000_000)
//!     .max_depth(100_000)
//!     .storage_mode(StorageMode::Exact)
//!     .search_mode(SearchMode::DepthFirst)
//!     .por_enabled(false)
//!     .check_assertions(true)
//!     .build();
//!
//! let result = checker.check(); // or check_dfs() / check_bfs()
//! println!("States explored: {}", result.states_explored);
//! ```
//!
//! ## LTL Verification
//!
//! ```rust,no_run
//! use spin_rs::property::{verify_ltl, LtlFormula};
//!
//! let promela = r#"
//!     active proctype P() {
//!         byte x = 0;
//!         do
//!         :: x = 0
//!         :: x = 1
//!         od
//!     }
//! "#;
//!
//! let formula = "[]<>(x == 0)"; // Always eventually x=0
//! let violation = verify_ltl(promela, formula, "liveness").unwrap();
//! if let Some(v) = violation {
//!     println!("Property violated: {}", v.property_name);
//! } else {
//!     println!("Property holds");
//! }
//! ```
//!
//! ## Architecture
//!
//! ```text
//! Promela source
//!     │
//!     ▼
//! parser::parse() ──► PromelaModel (AST)
//!     │
//!     ▼
//! codegen::generate() ──► GeneratedLua
//!     │
//!     ▼
//! runtime::LuaModel ──► impl Model trait
//!     │
//!     ▼
//! engine::Checker ──► DFS/BFS exploration
//!     │
//!     ▼
//! CheckResult { states, transitions, errors, violations }
//! ```
//!
//! ## Features
//!
//! - `lua-runtime` (default): Enable Lua runtime via mlua
//!
//! ## Limitations (v1)
//!
//! - Promela subset: no embedded C, d_step, remote refs, priority
//! - LTL → Büchi: simplified (full ω-automata integration in v2)
//! - POR: basic persistent sets (advanced POR in v2)

pub mod cli;
pub mod codegen;
pub mod engine;
pub mod parser;
pub mod por;
pub mod property;
pub mod runtime;
pub mod trail;

// Re-export core types
pub use engine::checker::CheckerBuilder;
pub use engine::checker::{
    CheckResult, Checker, CheckerConfig, Model, SearchMode, StorageMode, Transition,
};
pub use parser::ast::PromelaModel;

// Re-export runtime types
pub use runtime::{LuaModel, LuaRuntime, verify as verify_model};

// Re-export property types
pub use property::buchi::{BuchiAutomaton, BuchiTransition, ProductState, ProductTransition};
pub use property::{LtlFormula, PropertyChecker, verify_ltl};

// Re-export trail types
pub use trail::{ErrorTrail, TrailReplayer, TrailStats, TrailStep};

/// Convenience function: parse Promela, generate Lua, run verification.
///
/// This is the simplest way to verify a Promela model.
///
/// # Example
///
/// ```rust
/// use spin_rs::verify;
///
/// let promela = "active proctype P() { byte x; x = 1; }";
/// let result = verify(promela).unwrap();
/// assert_eq!(result.errors, 0);
/// ```
pub fn verify(source: &str) -> anyhow::Result<CheckResult> {
    let model = LuaModel::from_source(source)?;
    let checker = CheckerBuilder::new().model(model).build();
    Ok(checker.check())
}

/// Verify with custom configuration.
///
/// # Example
///
/// ```rust
/// use spin_rs::{verify_with_config, CheckerConfig, StorageMode};
///
/// let promela = "active proctype P() { byte x; x = 1; }";
/// let config = CheckerConfig {
///     max_states: 100_000,
///     storage_mode: StorageMode::Bitstate,
///     ..Default::default()
/// };
/// let result = verify_with_config(promela, &config).unwrap();
/// ```
pub fn verify_with_config(source: &str, config: &CheckerConfig) -> anyhow::Result<CheckResult> {
    let model = LuaModel::from_source(source)?;
    let builder = CheckerBuilder::new()
        .model(model)
        .max_states(config.max_states)
        .max_depth(config.max_depth)
        .storage_mode(config.storage_mode)
        .search_mode(config.search_mode)
        .por_enabled(config.por_enabled)
        .check_assertions(config.check_assertions);

    let checker = builder.build();
    Ok(checker.check())
}

/// Parse Promela source into an AST.
///
/// # Example
///
/// ```rust
/// use spin_rs::parse;
///
/// let promela = "active proctype P() { byte x; }";
/// let ast = parse(promela).unwrap();
/// assert!(!ast.declarations.is_empty());
/// ```
pub fn parse(source: &str) -> anyhow::Result<PromelaModel> {
    parser::parse(source)
}

/// Generate Lua code from a Promela AST.
///
/// # Example
///
/// ```rust
/// use spin_rs::{parse, generate_lua};
///
/// let promela = "active proctype P() { byte x; }";
/// let ast = parse(promela).unwrap();
/// let lua = generate_lua(&ast);
/// assert!(lua.source.contains("function _spin_init_state"));
/// ```
pub fn generate_lua(model: &PromelaModel) -> codegen::GeneratedLua {
    codegen::generate(model)
}

/// Create a model from Promela source.
///
/// # Example
///
/// ```rust
/// use spin_rs::create_model;
///
/// let promela = "active proctype P() { byte x; }";
/// let model = create_model(promela).unwrap();
/// ```
pub fn create_model(source: &str) -> anyhow::Result<LuaModel> {
    LuaModel::from_source(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_simple() {
        let promela = "active proctype P() { byte x; x = 1; }";
        let result = verify(promela).unwrap();
        assert!(result.states_explored > 0);
        assert_eq!(result.errors, 0);
    }

    #[test]
    fn test_verify_with_assertion() {
        let promela = r#"
            active proctype P() {
                byte x = 0;
                x = 1;
                assert(x == 1);
            }
        "#;
        let result = verify(promela).unwrap();
        assert_eq!(result.errors, 0);
    }

    #[test]
    fn test_parse_and_generate() {
        let promela = "active proctype P() { byte x; }";
        let ast = parse(promela).unwrap();
        let lua = generate_lua(&ast);
        assert!(lua.source.contains("_spin_init_state"));
    }

    #[test]
    fn test_create_model() {
        let promela = "active proctype P() { byte x; }";
        let model = create_model(promela).unwrap();
        let init_states = model.init_states();
        assert!(!init_states.is_empty());
    }

    #[test]
    fn test_verify_with_config() {
        let promela = "active proctype P() { byte x; x = 1; }";
        let config = CheckerConfig {
            max_states: 10_000,
            max_depth: 1_000,
            storage_mode: StorageMode::Exact,
            search_mode: SearchMode::DepthFirst,
            por_enabled: false,
            check_assertions: true,
        };
        let result = verify_with_config(promela, &config).unwrap();
        assert!(result.states_explored > 0);
    }
}
