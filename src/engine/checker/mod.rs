//! Model checker engine: DFS/BFS state exploration with multiple storage backends.

pub mod deps;
pub mod dfs;
mod por_run;

pub use deps::{TransitionDeps, ample_set};

#[cfg(test)]
mod por_tests;
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

    /// Decide an atomic proposition that names no single state variable, such as `a == b`.
    ///
    /// `atomic_props` answers per state field, which covers a bare identifier and nothing else.
    /// A comparison used to be read as "absent" and therefore false, which made the product
    /// transition disabled and the claim undecided - or, worse, vacuously satisfied. Returning
    /// None means "not decided" and the caller must not read it as false.
    fn eval_atom(&self, _state: &Self::State, _atom: &str) -> Option<bool> {
        None
    }

    /// Claims the model declares that this engine cannot check at all.
    ///
    /// A construct that is emitted and never runs is worse than one that is refused: the model
    /// comes back verified. Reporting it here means the run says "no verdict" instead.
    fn unchecked_claims(&self) -> Vec<(String, String)> {
        Vec::new()
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

    /// The claims this model can be reduced per.
    ///
    /// Empty means "no reduction metadata"; the checker then runs exactly one unreduced
    /// traversal, which is the pre-reduction behaviour and correct for every model.
    fn reduction_scopes(&self) -> Vec<ReductionScope> {
        Vec::new()
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
    /// What this transition reads and writes. Opaque by default, so a model that does not
    /// describe its transitions is never reduced.
    pub deps: TransitionDeps,
}

impl<S> Transition<S> {
    /// Build a transition whose dependencies are unknown, hence conservative.
    pub fn new(label: impl Into<String>, next: S) -> Self {
        Self {
            label: label.into(),
            next,
            deps: TransitionDeps::default(),
        }
    }

    /// Attach declared dependencies.
    pub fn with_deps(mut self, deps: TransitionDeps) -> Self {
        self.deps = deps;
        self
    }
}

/// One claim's view of the model, for reduction purposes.
///
/// Reduction is only sound relative to what a property can observe, so each claim gets its own
/// traversal with its own visible variable set. A claim with an empty visible set would be
/// reduced against a blind property - which is how a reduced search hides a violation - so an
/// unresolvable claim declares everything visible instead.
#[derive(Debug, Clone)]
pub struct ReductionScope {
    /// Claim name, used to attribute findings and budget shortfalls.
    pub name: String,
    /// State fields whose change can alter this claim's truth value.
    pub visible: std::collections::BTreeSet<String>,
    /// True when the claim is a liveness property. Reduction is disabled for these: an accepting
    /// cycle can be invisible, so hiding one loses the violation.
    pub liveness: bool,
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
    /// Bounds the whole run, not each claim.
    ///
    /// Reduction checks one claim per traversal, so a run may make several traversals. Claims
    /// are taken in declaration order and each may use whatever budget remains, which means an
    /// expensive early claim can starve the later ones. Those are reported in `unverified` with
    /// kind `StateBudget` naming the claim - never silently dropped.
    pub max_states: usize,
    /// Depth limit, applied per traversal: each claim's traversal starts again at depth 0.
    pub max_depth: usize,
    pub storage_mode: StorageMode,
    pub search_mode: SearchMode,
    /// Enable partial order reduction. On by default; a model that declares no claims, or whose
    /// transitions carry no dependency metadata, is explored unreduced either way.
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
            por_enabled: true,
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
    /// States this claim's own traversal stored.
    ///
    /// A caller deciding how much budget to give a plan needs to know whether a claim came four
    /// thousand states short or four million. The reason string said "used up" and nothing else.
    pub states_used: usize,
    /// The ceiling that traversal was given, when it had one.
    pub budget: Option<usize>,
}

impl Unverified {
    /// An unverified entry with no measured usage.
    pub fn new(
        property_name: impl Into<String>,
        kind: UnverifiedKind,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            property_name: property_name.into(),
            kind,
            reason: reason.into(),
            states_used: 0,
            budget: None,
        }
    }

    /// Record what the claim's traversal used and what it was allowed.
    pub fn with_usage(mut self, states_used: usize, budget: Option<usize>) -> Self {
        self.states_used = states_used;
        self.budget = budget;
        self
    }
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
    /// States stored across every traversal the run made.
    ///
    /// With reduction this is a **sum over per-claim traversals**, not the size of one reachable
    /// state space: a model with two claims may report twice the states of a single unreduced
    /// pass. It is the same quantity as summing pan's per-claim runs, and it is not comparable
    /// with the pre-reduction value of this field.
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
    /// Add the claims this engine does not check, so a run cannot report success without them.
    fn with_unchecked_claims(&self, mut result: CheckResult) -> CheckResult {
        for (name, reason) in self.model.unchecked_claims() {
            result.unverified.push(Unverified::new(
                name,
                UnverifiedKind::UnsupportedProperty,
                reason,
            ));
        }
        result
    }

    /// Run the model checker with the configured search mode.
    pub fn check(&self) -> CheckResult {
        // Reduction needs a per-claim visible set, which a model without claims cannot supply;
        // it is also a DFS technique, so BFS keeps its single unreduced traversal.
        let result = if self.config.por_enabled
            && self.config.search_mode == SearchMode::DepthFirst
            && !self.model.reduction_scopes().is_empty()
        {
            self.check_dfs_por()
        } else {
            match self.config.search_mode {
                SearchMode::DepthFirst => self.check_dfs(),
                SearchMode::BreadthFirst => self.check_bfs(),
            }
        };
        self.with_unchecked_claims(result)
    }
}
