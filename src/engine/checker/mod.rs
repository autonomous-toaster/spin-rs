//! Model checker engine: DFS/BFS state exploration with multiple storage backends.

pub mod dfs;

#[cfg(test)]
mod tests;

use crate::engine::fairness::FairnessMode;
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

    /// Resolve a state to its atomic proposition values.
    ///
    /// Returns `None` when the model cannot expose variable values. Callers must not
    /// invent defaults in that case: an unresolved proposition previously became
    /// `false`, which made implication-shaped properties vacuously true.
    fn atomic_props(
        &self,
        _state: &Self::State,
    ) -> Option<std::collections::HashMap<String, bool>> {
        None
    }

    /// Number of process instances the model declares, or 0 if unknown.
    ///
    /// Used to reject a search that explored nothing. A model with processes but no
    /// transitions is a checker or parser failure, never a successful verification.
    fn process_count(&self) -> usize {
        0
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
    /// Use hash-compact storage: 64-bit hashes with LRU cache for collision detection.
    HashCompact,
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
    pub fairness_mode: FairnessMode,
    /// Stop the search as soon as one violation is found.
    ///
    /// Off by default: the exhaustive search reports every violated property, which is what a
    /// caller needs to attribute findings per claim. Turning it on matches `pan`'s default and
    /// costs the prefix up to the first refutation instead of the whole reachable space, but
    /// any property not reported was then not fully explored - treat the remainder as
    /// unverified, not as satisfied.
    pub stop_at_first_violation: bool,
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
            fairness_mode: FairnessMode::None,
            stop_at_first_violation: false,
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

/// A property that was not decided, and why.
///
/// This is distinct from a violation: a violation is a definite refutation, while an
/// unverified property means the checker could not produce a verdict at all. Callers must
/// not map it to "invalid". It also covers a search that stopped at `max_states` before the
/// state space closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum UnverifiedKind {
    /// The property uses an operator, or a nesting, the automaton builder cannot complement.
    UnsupportedProperty,
    /// The formula did not parse.
    UnparsableFormula,
    /// The search stopped at `max_states` before the state space closed.
    StateBudget,
    /// The search stopped at `max_depth`.
    DepthBudget,
    /// The search stopped at the first violation, so this claim was never fully explored.
    EarlyStop,
}

impl UnverifiedKind {
    /// Stable machine-readable tag, safe to classify on instead of matching messages.
    pub fn as_str(&self) -> &'static str {
        match self {
            UnverifiedKind::UnsupportedProperty => "unsupported_property",
            UnverifiedKind::UnparsableFormula => "unparsable_formula",
            UnverifiedKind::StateBudget => "state_budget",
            UnverifiedKind::DepthBudget => "depth_budget",
            UnverifiedKind::EarlyStop => "early_stop",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Unverified {
    /// Name of the property, or "model" for a whole-model limitation.
    pub property_name: String,
    /// Why no verdict was produced, as a stable tag rather than only prose.
    pub kind: UnverifiedKind,
    /// Human-readable detail.
    pub reason: String,
}

/// Verification result.
///
/// # The invariant this type exists to protect
///
/// **Every claim must appear in `violations` or in `unverified`. Absence must never be allowed
/// to mean anything.**
///
/// Three separate defects in this checker had the same root, and each one produced a wrong
/// verdict that looked like a clean run:
///
/// - a budget was pruned and never reported, so a truncated search read as a completed one;
/// - a liveness violation was reported under a generic label instead of its claim, so a caller
///   that attributes by name dropped it and counted the requirement as satisfied;
/// - an early stop simply omitted the claims it never reached, so "no violation reported" was
///   read as "satisfied".
///
/// When adding a way to skip work - a budget, a depth cut, an early stop, a new unsupported
/// shape - the claim being skipped must land in `unverified` with a `kind`. A comment is not
/// enough: the property is asserted by tests, because the failure mode is invisible at the
/// call site and only shows up as a false green downstream.
#[derive(Debug, Clone)]
pub struct CheckResult {
    pub states_explored: usize,
    pub states_stored: usize,
    pub transitions: usize,
    pub depth_reached: usize,
    pub errors: usize,
    pub violations: Vec<Violation>,
    /// Properties that produced no verdict. Non-empty means the run was incomplete, so
    /// `errors == 0` does not license a success claim.
    pub unverified: Vec<Unverified>,
    pub elapsed_secs: f64,
}

/// A model the checker could not explore or evaluate.
///
/// Returned as the payload of the `Err` from `verify` and `verify_with_config`, so a caller can
/// classify it with `downcast_ref` instead of matching message text. Every variant means
/// "no verdict", never "invalid".
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum VerifyError {
    /// No process was enabled in the initial state, so nothing was explored.
    NotExplored { processes: usize },
    /// A generated guard or effect failed to evaluate.
    EvalFailed { detail: String },
}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerifyError::NotExplored { processes } => write!(
                f,
                "no transitions explored: the model declares {} process(es) but none are enabled in the initial state, so nothing was verified",
                processes
            ),
            VerifyError::EvalFailed { detail } => {
                write!(f, "transition evaluation failed: {}", detail)
            }
        }
    }
}

impl std::error::Error for VerifyError {}

impl VerifyError {
    /// Stable machine-readable tag, safe to classify on.
    pub fn kind(&self) -> &'static str {
        match self {
            VerifyError::NotExplored { .. } => "not_explored",
            VerifyError::EvalFailed { .. } => "eval_failed",
        }
    }
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

    pub fn fairness_mode(mut self, mode: FairnessMode) -> Self {
        self.config.fairness_mode = mode;
        self
    }

    pub fn stop_at_first_violation(mut self, enabled: bool) -> Self {
        self.config.stop_at_first_violation = enabled;
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
