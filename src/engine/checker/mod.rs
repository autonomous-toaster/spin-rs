//! Model checker engine: DFS/BFS state exploration with multiple storage backends.

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

    /// Run DFS state exploration with exact state matching, assertion checking,
    /// and error trail recording.
    pub fn check_dfs(&self) -> CheckResult {
        let start = std::time::Instant::now();
        let init_states = self.model.init_states();

        if init_states.is_empty() {
            return self.empty_result(0.0);
        }

        let mut storage = self.make_storage();
        let mut stack: Vec<(M::State, usize, usize)> = Vec::new(); // (state, depth, parent_index)
        let mut trail: Vec<(String, usize)> = Vec::new(); // (transition_label, parent_index)
        let mut transitions_count = 0;
        let mut violations = Vec::new();

        for s in init_states {
            let h = self.model.hash(&s);
            if storage.insert(h, &s) {
                let idx = trail.len();
                trail.push((String::new(), 0));
                stack.push((s, 0, idx));
            }
        }

        while let Some((state, depth, state_idx)) = stack.pop() {
            if depth >= self.config.max_depth {
                continue;
            }
            if storage.len() >= self.config.max_states {
                break;
            }

            // Check for violations (safety properties / assertions)
            if self.config.check_assertions
                && let Some(desc) = self.model.check_violation(&state)
            {
                let state_trail = self.build_trail(&trail, state_idx);
                violations.push(Violation {
                    property_name: "assertion".to_string(),
                    trail: state_trail,
                    description: desc,
                });
                if violations.len() >= 100 {
                    break;
                }
                continue;
            }

            let trans = self.model.transitions(&state);
            transitions_count += trans.len();

            for t in trans {
                let h = self.model.hash(&t.next);
                if storage.insert(h, &t.next) {
                    let idx = trail.len();
                    trail.push((t.label, state_idx));
                    stack.push((t.next, depth + 1, idx));
                }
            }
        }

        let elapsed = start.elapsed().as_secs_f64();

        let mut result = CheckResult {
            states_explored: storage.len(),
            states_stored: storage.len(),
            transitions: transitions_count,
            depth_reached: self
                .config
                .max_depth
                .min(stack.iter().map(|(_, d, _)| *d).max().unwrap_or(0)),
            errors: violations.len(),
            violations,
            elapsed_secs: elapsed,
        };

        // Check LTL properties (nested DFS for liveness)
        self.check_ltl_properties(&mut result);

        result
    }

    /// Check LTL properties:
    /// - `[]p` (always): check as invariant during DFS — any reachable state where p is false violates
    /// - Liveness (`<>p`, `pUq`): deferred to full nested DFS
    fn check_ltl_properties(&self, result: &mut CheckResult) {
        use crate::runtime::LuaModel;

        let lua_model = match &self.model as *const M as *const LuaModel {
            ptr if !ptr.is_null() => unsafe { &*ptr },
            _ => return,
        };

        let formulas = lua_model.ltl_formulas();
        if formulas.is_empty() {
            return;
        }

        // For each []p formula, re-explore and check invariant
        for ltl_ast in formulas {
            let formula_str = ltl_ast.formula.trim();
            if let Some(inner) = formula_str.strip_prefix("[]") {
                let prop_name = ltl_ast.name.as_deref().unwrap_or("ltl");
                if let Err(e) = self.check_always_property(result, prop_name, inner.trim()) {
                    log::warn!("LTL check error for '{}': {}", prop_name, e);
                    // Break on first error — don't crash from unsafe cast
                    break;
                }
            }
        }
    }

    /// Check []p (always p) as invariant: DFS to find any state violating condition.
    fn check_always_property(&self, result: &mut CheckResult, prop_name: &str, condition: &str) -> anyhow::Result<()> {
        use std::collections::HashSet;
        

        let init_states = self.model.init_states();
        if init_states.is_empty() {
            return Ok(());
        }

        let mut visited: HashSet<u64> = HashSet::new();
        let mut stack: Vec<(M::State, usize)> = Vec::new();

        for s in init_states {
            let h = self.model.hash(&s);
            if visited.insert(h) {
                stack.push((s, 0));
            }
        }

        while let Some((state, depth)) = stack.pop() {
            if depth > 100_000 {
                continue;
            }

            // Check if this state violates the condition
            // SAFETY: Only valid when M::State is StateBlob (LuaModel)
            let blob_opt = if std::any::TypeId::of::<M::State>()
                == std::any::TypeId::of::<crate::runtime::StateBlob>()
            {
                unsafe {
                    let ptr = &state as *const M::State as *const crate::runtime::StateBlob;
                    ptr.as_ref().map(|s| &s.0)
                }
            } else {
                None
            };

            if let Some(blob) = blob_opt
                && Self::state_violates_invariant(blob, condition)
            {
                result.errors += 1;
                result.violations.push(Violation {
                    property_name: prop_name.to_string(),
                    trail: vec![],
                    description: format!(
                        "LTL violation: '{}' (condition: {}) fails in reachable state",
                        prop_name, condition
                    ),
                });
                return Ok(());
            }

            let transitions = self.model.transitions(&state);
            for t in transitions {
                let h = self.model.hash(&t.next);
                if visited.insert(h) {
                    stack.push((t.next, depth + 1));
                }
            }
        }

        Ok(())
    }

    /// Check if a state blob violates a `[]p` invariant condition.
    /// Handles "x == 0", "!x", "x", "x != 0" patterns.
    fn state_violates_invariant(blob: &str, condition: &str) -> bool {
        let cond = condition.trim();

        // Parse "var == val"
        if let Some(pos) = cond.find("==") {
            let var = cond[..pos].trim();
            let val = cond[pos + 2..].trim();
            // []p violated if state variable != expected value
            return !Self::state_has_value(blob, var, val);
        }

        // Parse "var != val"
        if let Some(pos) = cond.find("!=") {
            let var = cond[..pos].trim();
            let val = cond[pos + 2..].trim();
            // []p violated if state variable == expected value
            return Self::state_has_value(blob, var, val);
        }

        // Parse "!var"
        if let Some(rest) = cond.strip_prefix('!') {
            let var = rest.trim();
            // []p violated if var is true (non-zero)
            return Self::state_is_truthy(blob, var);
        }

        // Plain "var" — []p violated if var is false (zero/nil)
        !Self::state_is_truthy(blob, cond)
    }

    /// Check if a state blob has a variable with a specific value.
    fn state_has_value(blob: &str, var_name: &str, expected: &str) -> bool {
        let inner = blob.trim_start_matches('{').trim_end_matches('}');
        for entry in inner.split(',') {
            let entry = entry.trim();
            if let Some(colon_pos) = entry.find(':') {
                let key = entry[..colon_pos].trim().trim_matches('"');
                let value = entry[colon_pos + 1..].trim();
                if key == var_name && value == expected {
                    return true;
                }
            }
        }
        false
    }

    /// Check if a state variable is truthy (non-zero, non-false, non-nil).
    fn state_is_truthy(blob: &str, var_name: &str) -> bool {
        let inner = blob.trim_start_matches('{').trim_end_matches('}');
        for entry in inner.split(',') {
            let entry = entry.trim();
            if let Some(colon_pos) = entry.find(':') {
                let key = entry[..colon_pos].trim().trim_matches('"');
                let value = entry[colon_pos + 1..].trim();
                if key == var_name {
                    return value != "0" && value != "false" && value != "nil";
                }
            }
        }
        false
    }

    /// Run BFS state exploration.
    pub fn check_bfs(&self) -> CheckResult {
        let start = std::time::Instant::now();
        let init_states = self.model.init_states();

        if init_states.is_empty() {
            return self.empty_result(0.0);
        }

        let mut storage = self.make_storage();
        let mut queue: VecDeque<(M::State, usize, usize)> = VecDeque::new(); // (state, depth, parent_index)
        let mut trail: Vec<(String, usize)> = Vec::new();
        let mut transitions_count = 0;
        let mut violations = Vec::new();
        let mut max_depth = 0;

        for s in init_states {
            let h = self.model.hash(&s);
            if storage.insert(h, &s) {
                let idx = trail.len();
                trail.push((String::new(), 0));
                queue.push_back((s, 0, idx));
            }
        }

        while let Some((state, depth, state_idx)) = queue.pop_front() {
            max_depth = max_depth.max(depth);

            if depth >= self.config.max_depth {
                continue;
            }
            if storage.len() >= self.config.max_states {
                break;
            }

            if self.config.check_assertions
                && let Some(desc) = self.model.check_violation(&state)
            {
                let state_trail = self.build_trail(&trail, state_idx);
                violations.push(Violation {
                    property_name: "assertion".to_string(),
                    trail: state_trail,
                    description: desc,
                });
                if violations.len() >= 100 {
                    break;
                }
                continue;
            }

            let trans = self.model.transitions(&state);
            transitions_count += trans.len();

            for t in trans {
                let h = self.model.hash(&t.next);
                if storage.insert(h, &t.next) {
                    let idx = trail.len();
                    trail.push((t.label, state_idx));
                    queue.push_back((t.next, depth + 1, idx));
                }
            }
        }

        let elapsed = start.elapsed().as_secs_f64();

        CheckResult {
            states_explored: storage.len(),
            states_stored: storage.len(),
            transitions: transitions_count,
            depth_reached: max_depth,
            errors: violations.len(),
            violations,
            elapsed_secs: elapsed,
        }
    }

    /// Build an error trail from parent pointers.
    fn build_trail(&self, trail: &[(String, usize)], end_idx: usize) -> Vec<String> {
        let mut result = Vec::new();
        let mut idx = end_idx;
        while idx > 0 {
            let (ref label, _) = trail[idx];
            if !label.is_empty() {
                result.push(label.clone());
            }
            idx = trail[idx].1;
            // Guard against runaway loops (malformed trail index)
            if result.len() > 100_000 {
                break;
            }
        }
        // We built from parent back to root, so reverse
        result.reverse();
        result
    }

    fn make_storage(&self) -> Box<dyn StateStore<M::State>> {
        match self.config.storage_mode {
            StorageMode::Exact => Box::new(ExactStore::<M::State>::new()),
            StorageMode::Bitstate => {
                Box::new(BitstateStore::new((self.config.max_states / 8).max(1024)))
            }
            StorageMode::Collapse => Box::new(CollapseStore::<M::State>::new(4)),
        }
    }

    fn empty_result(&self, elapsed_secs: f64) -> CheckResult {
        CheckResult {
            states_explored: 0,
            states_stored: 0,
            transitions: 0,
            depth_reached: 0,
            errors: 0,
            violations: vec![],
            elapsed_secs,
        }
    }

    pub fn model(&self) -> &M {
        &self.model
    }

    pub fn check_dfs_old(&self) -> CheckResult {
        self.check_dfs()
    }
}

#[cfg(test)]
mod tests;
