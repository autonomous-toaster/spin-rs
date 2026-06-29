//! Model checker engine: DFS/BFS state exploration with multiple storage backends.

pub mod dfs;

use crate::engine::storage::{BitstateStore, CollapseStore, ExactStore, StateStore};
use std::collections::VecDeque;

use std::hash::Hash;

/// A model-specific state transition system.
pub trait Model {
    type State: Hash + Eq + Clone + Send + 'static;

    /// Generate all initial states.
    fn init_states(&self) -> Vec<Self::State>;

    /// Enumerate all enabled transitions from a given state.
    fn transitions(&self, state: &Self::State) -> Vec<Transition<Self::State>>;

    /// Compute a hash fingerprint of a state.
    fn hash(&self, state: &Self::State) -> u64;

    /// Check whether a state violates a safety property.
    /// Returns `None` if ok, `Some(description)` if violated.
    fn check_violation(&self, _state: &Self::State) -> Option<String> {
        None
    }

    /// Return LTL formulas associated with this model (empty by default).
    fn ltl_formulas(&self) -> &[crate::parser::ast::LtlFormula] {
        &[]
    }

    /// Serialize a state to a string for invariant checking (None if not supported).
    fn state_to_string(&self, _state: &Self::State) -> Option<String> {
        None
    }
}

/// A single transition from one state to another.
#[derive(Debug, Clone)]
pub struct Transition<S> {
    pub label: String,
    pub next: S,
}

/// State storage mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StorageMode {
    /// Store full state vectors in a hash table with collision resolution.
    Exact,
    /// Use bitstate hashing (Bloom filter with two hashes).
    Bitstate,
    /// Use collapse compression (per-component canonical ordinals).
    Collapse,
}

/// Search mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SearchMode {
    DepthFirst,
    BreadthFirst,
}

/// Configuration for the model checker.
#[derive(Debug, Clone)]
pub struct CheckerConfig {
    pub max_states: usize,
    pub max_depth: usize,
    pub storage_mode: StorageMode,
    pub search_mode: SearchMode,
    pub por_enabled: bool,
    pub check_assertions: bool,
}

impl Default for CheckerConfig {
    fn default() -> Self {
        Self {
            max_states: 1_000_000,
            max_depth: 100_000,
            storage_mode: StorageMode::Exact,
            search_mode: SearchMode::DepthFirst,
            por_enabled: false,
            check_assertions: true,
        }
    }
}

/// A property violation with error trail.
#[derive(Debug, Clone)]
pub struct Violation {
    pub property_name: String,
    pub trail: Vec<String>,
    pub description: String,
}

/// Verification result.
#[derive(Debug, Clone)]
pub struct CheckResult {
    pub states_explored: usize,
    pub states_stored: usize,
    pub transitions: usize,
    pub depth_reached: usize,
    pub errors: usize,
    pub violations: Vec<Violation>,
    pub elapsed_secs: f64,
}

// ─── Checker Builder ───────────────────────────────────────────

/// Builder for constructing and running a model checker.
#[derive(Debug)]
pub struct CheckerBuilder<M: Model> {
    model: Option<M>,
    config: CheckerConfig,
}

impl<M: Model> CheckerBuilder<M> {
    pub fn new() -> Self {
        Self {
            model: None,
            config: CheckerConfig::default(),
        }
    }

    pub fn model(mut self, model: M) -> Self {
        self.model = Some(model);
        self
    }

    pub fn max_states(mut self, n: usize) -> Self {
        self.config.max_states = n;
        self
    }

    pub fn max_depth(mut self, n: usize) -> Self {
        self.config.max_depth = n;
        self
    }

    pub fn storage_mode(mut self, mode: StorageMode) -> Self {
        self.config.storage_mode = mode;
        self
    }

    pub fn search_mode(mut self, mode: SearchMode) -> Self {
        self.config.search_mode = mode;
        self
    }

    pub fn por_enabled(mut self, enabled: bool) -> Self {
        self.config.por_enabled = enabled;
        self
    }

    pub fn check_assertions(mut self, enabled: bool) -> Self {
        self.config.check_assertions = enabled;
        self
    }

    pub fn build(self) -> Checker<M> {
        Checker {
            model: self.model.expect("model must be set"),
            config: self.config,
        }
    }
}

impl<M: Model> Default for CheckerBuilder<M> {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Checker Engine ────────────────────────────────────────────

/// The model checker engine.
#[derive(Debug)]
pub struct Checker<M: Model> {
    model: M,
    config: CheckerConfig,
}

impl<M: Model> Checker<M> {
    /// Run the model checker with the configured search mode.
    pub fn check(&self) -> CheckResult {
        match self.config.search_mode {
            SearchMode::DepthFirst => self.check_dfs(),
            SearchMode::BreadthFirst => self.check_bfs(),
        }
    }
}
