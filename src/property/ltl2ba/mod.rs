//! Simplified LTL to Büchi automaton conversion.
//!
//! This module provides a simplified LTL → Büchi conversion that supports:
//! - Temporal operators: `[]` (always), `<>` (eventually), `X` (next)
//! - Boolean operators: `&&`, `||`, `!`, `->`
//! - Atomic propositions: variable comparisons (`x == 0`, `flag`)
//!
//! **Limitations**:
//! - Does NOT support `U` (until), `V` (release)
//! - Does NOT support nested temporal operators (`[]<>p`)
//! - Coverage: ~60-70% of real-world LTL properties
//!
//! For full LTL support, see the full ltl2ba implementation (future work).

pub mod buchi;
pub mod error;
pub mod formula;
pub mod nested_dfs;
pub mod parser;
pub mod product;

pub use buchi::{BuchiAutomaton, BuchiTransition, to_buchi};
pub use error::LtlError;
pub use formula::LtlFormula;
pub use nested_dfs::NestedDFS;
pub use parser::parse_ltl;
pub use product::{ProductState, ProductTransition, evaluate_atomic_props, sync_transitions};
