//! Model checker engine: DFS/BFS state exploration with multiple storage backends.

use std::collections::{HashMap, VecDeque};
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

// ─── Storage Backends ──────────────────────────────────────────

/// Trait abstracting state storage (visited set).
pub trait StateStore<S> {
    /// Insert a state. Returns `true` if the state was newly inserted (not a duplicate).
    fn insert(&mut self, hash: u64, state: &S) -> bool;
    /// Number of stored states.
    fn len(&self) -> usize;
}

/// Exact state storage with collision resolution.
pub struct ExactStore<S> {
    map: HashMap<u64, Vec<S>>,
    count: usize,
}

impl<S: Clone + Hash + Eq + Send> Default for ExactStore<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: Clone + Hash + Eq + Send> ExactStore<S> {
    pub fn new() -> Self {
        Self { map: HashMap::new(), count: 0 }
    }
}

impl<S: Clone + Hash + Eq + Send> StateStore<S> for ExactStore<S> {
    fn insert(&mut self, hash: u64, state: &S) -> bool {
        let bucket = self.map.entry(hash).or_default();
        if bucket.iter().any(|s| s == state) {
            return false; // duplicate
        }
        bucket.push(state.clone());
        self.count += 1;
        true
    }

    fn len(&self) -> usize {
        self.count
    }
}

/// Bitstate (Bloom filter) storage — fixed-size bitset with two hash functions.
pub struct BitstateStore {
    bits: Vec<u8>,
    bit_count: usize,
    set_count: usize,
}

impl BitstateStore {
    /// Create a bitstate store with `size` bytes.
    pub fn new(size: usize) -> Self {
        Self {
            bits: vec![0u8; size],
            bit_count: size * 8,
            set_count: 0,
        }
    }

    fn hash_index(hash: u64, offset: u64, modulo: usize) -> usize {
        let h = hash.wrapping_mul(offset);
        (h as usize) % modulo
    }
}

impl<S> StateStore<S> for BitstateStore {
    fn insert(&mut self, _hash: u64, _state: &S) -> bool {
        // Use two independent hash positions: h1 = hash, h2 = hash.wrapping_mul(0x9e3779b9)
        let h1 = Self::hash_index(_hash, 1, self.bit_count);
        let h2 = Self::hash_index(_hash, 0x9e3779b97f4a7c15, self.bit_count);

        let already_set = {
            let byte1 = h1 / 8;
            let bit1 = h1 % 8;
            let byte2 = h2 / 8;
            let bit2 = h2 % 8;
            (self.bits[byte1] & (1 << bit1)) != 0
                && (self.bits[byte2] & (1 << bit2)) != 0
        };

        if already_set {
            return false;
        }

        // Set both bits
        {
            let byte1 = h1 / 8;
            let bit1 = h1 % 8;
            let byte2 = h2 / 8;
            let bit2 = h2 % 8;
            self.bits[byte1] |= 1 << bit1;
            self.bits[byte2] |= 1 << bit2;
        }
        self.set_count += 1;
        true
    }

    fn len(&self) -> usize {
        self.set_count
    }
}

/// Collapse compression storage: canonical ordinals per component.
pub struct CollapseStore<S> {
    canonical: HashMap<u64, Vec<usize>>,
    _component_maps: Vec<HashMap<Vec<u8>, usize>>,
    count: usize,
    _marker: std::marker::PhantomData<S>,
}

impl<S: Clone + Hash + Eq + Send> CollapseStore<S> {
    pub fn new(components: usize) -> Self {
        Self {
            canonical: HashMap::new(),
            _component_maps: (0..components).map(|_| HashMap::new()).collect(),
            count: 0,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<S: Clone + Hash + Eq + Send> StateStore<S> for CollapseStore<S> {
    fn insert(&mut self, _hash: u64, _state: &S) -> bool {
        // For v1, fall through to exact-like behavior.
        // Full collapse will serialize each component separately.
        // This is a placeholder for the incremental implementation.
        let bucket = self.canonical.entry(_hash).or_default();
        // Without component serialization, we use a simple existence check.
        let exists = !bucket.is_empty();
        if !exists {
            bucket.push(self.count);
            self.count += 1;
            true
        } else {
            false
        }
    }

    fn len(&self) -> usize {
        self.count
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
        let mut trail: Vec<(String, u64)> = Vec::new(); // (transition_label, state_hash)
        let mut transitions_count = 0;
        let mut violations = Vec::new();

        for s in init_states {
            let h = self.model.hash(&s);
            if storage.insert(h, &s) {
                let idx = trail.len();
                trail.push((String::new(), h));
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
                && let Some(desc) = self.model.check_violation(&state) {
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
                    trail.push((t.label, h));
                    stack.push((t.next, depth + 1, idx));
                }
            }
        }

        let elapsed = start.elapsed().as_secs_f64();

        CheckResult {
            states_explored: storage.len(),
            states_stored: storage.len(),
            transitions: transitions_count,
            depth_reached: self.config.max_depth.min(
                stack.iter().map(|(_, d, _)| *d).max().unwrap_or(0)
            ),
            errors: violations.len(),
            violations,
            elapsed_secs: elapsed,
        }
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
        let mut trail: Vec<(String, u64)> = Vec::new();
        let mut transitions_count = 0;
        let mut violations = Vec::new();
        let mut max_depth = 0;

        for s in init_states {
            let h = self.model.hash(&s);
            if storage.insert(h, &s) {
                let idx = trail.len();
                trail.push((String::new(), h));
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
                && let Some(desc) = self.model.check_violation(&state) {
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
                    trail.push((t.label, h));
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
    fn build_trail(&self, trail: &[(String, u64)], end_idx: usize) -> Vec<String> {
        let mut result = Vec::new();
        let mut idx = end_idx;
        while idx > 0 {
            let (ref label, _) = trail[idx];
            if !label.is_empty() {
                result.push(label.clone());
            }
            idx = trail[idx].1 as usize;
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
            StorageMode::Bitstate => Box::new(BitstateStore::new(
                (self.config.max_states / 8).max(1024),
            )),
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
mod tests {
    use super::*;

    /// A simple test model with 3 states in a chain: A → B → C (self-loop).
    struct ChainModel;

    impl Model for ChainModel {
        type State = u8;

        fn init_states(&self) -> Vec<u8> {
            vec![0]
        }

        fn transitions(&self, state: &u8) -> Vec<Transition<u8>> {
            match state {
                0 => vec![Transition { label: "A→B".into(), next: 1 }],
                1 => vec![Transition { label: "B→C".into(), next: 2 }],
                2 => vec![Transition { label: "C→C".into(), next: 2 }],
                _ => vec![],
            }
        }

        fn hash(&self, state: &u8) -> u64 {
            *state as u64
        }
    }

    #[test]
    fn test_dfs_chain() {
        let model = ChainModel;
        let checker = CheckerBuilder::new().model(model).build();
        let result = checker.check_dfs();
        assert_eq!(result.states_explored, 3);
        assert_eq!(result.transitions, 3); // A→B, B→C, C→C
        assert_eq!(result.errors, 0);
    }

    #[test]
    fn test_bfs_chain() {
        let model = ChainModel;
        let checker = CheckerBuilder::new()
            .model(model)
            .search_mode(SearchMode::BreadthFirst)
            .build();
        let result = checker.check_bfs();
        assert_eq!(result.states_explored, 3);
    }

    #[test]
    fn test_exact_store_collision() {
        let mut store = ExactStore::new();
        // Same hash, different states
        assert!(store.insert(42, &"state_a".to_string()));
        assert!(store.insert(42, &"state_b".to_string()));
        assert!(!store.insert(42, &"state_a".to_string())); // duplicate
        assert!(!store.insert(42, &"state_b".to_string())); // duplicate
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn test_bitstate_store() {
        let mut store = BitstateStore::new(256);
        assert!(store.insert(0, &"hello".to_string()));
        // Same hash should be detected as seen
        assert!(!store.insert(0, &"hello".to_string()));
        // Although bitstate can have false positives, different hashes should be new
        assert!(store.insert(1, &"world".to_string()));
    }

    #[test]
    fn test_max_depth_limit() {
        let model = ChainModel;
        let checker = CheckerBuilder::new()
            .model(model)
            .max_depth(1)
            .build();
        let result = checker.check_dfs();
        // Should only explore to depth 1: state 0 → state 1
        assert_eq!(result.states_explored, 2);
    }

    #[test]
    fn test_max_states_limit() {
        let model = ChainModel;
        let checker = CheckerBuilder::new()
            .model(model)
            .max_states(2)
            .build();
        let result = checker.check_dfs();
        assert_eq!(result.states_explored, 2);
    }

    /// A model that violates an assertion at a specific state.
    struct ViolationModel;

    impl Model for ViolationModel {
        type State = i32;

        fn init_states(&self) -> Vec<i32> {
            vec![0]
        }

        fn transitions(&self, state: &i32) -> Vec<Transition<i32>> {
            match state {
                0 => vec![Transition { label: "0→1".into(), next: 1 }],
                1 => vec![Transition { label: "1→2".into(), next: 2 }],
                _ => vec![Transition { label: "loop".into(), next: *state }],
            }
        }

        fn hash(&self, state: &i32) -> u64 {
            *state as u64
        }

        fn check_violation(&self, state: &i32) -> Option<String> {
            if *state == 2 {
                Some("state 2 is forbidden".to_string())
            } else {
                None
            }
        }
    }

    #[test]
    fn test_violation_detection() {
        let model = ViolationModel;
        let checker = CheckerBuilder::new().model(model).build();
        let result = checker.check_dfs();
        assert_eq!(result.errors, 1);
        assert_eq!(result.violations[0].description, "state 2 is forbidden");
        assert!(!result.violations[0].trail.is_empty());
    }

    #[test]
    fn test_violation_with_trail() {
        let model = ViolationModel;
        let checker = CheckerBuilder::new().model(model).build();
        let result = checker.check_dfs();
        let trail = &result.violations[0].trail;
        // Trail should include the transitions that lead to state 2
        assert!(trail.contains(&"0→1".to_string()) || trail.contains(&"1→2".to_string()));
    }
}
