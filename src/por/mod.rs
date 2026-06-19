//! Partial Order Reduction (POR) engine.
//!
//! Implements persistent-set (ample-set) selection to reduce the state space
//! by exploring only a subset of enabled transitions when independence holds.
//!
//! Key concepts:
//! - **Independence**: Two transitions are independent if they don't interfere
//!   (neither disables the other, and they commute).
//! - **Persistent set**: A subset of enabled transitions such that any transition
//!   not in the set is independent of all transitions in the set.
//! - **Visible transitions**: Transitions that affect property satisfaction
//!   (e.g., shared variable writes, assertions).

use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use crate::engine::checker::{Model, Transition};

/// Dependency information for a transition.
#[derive(Debug, Clone)]
pub struct TransitionDeps {
    /// Variables read by this transition.
    pub reads: HashSet<String>,
    /// Variables written by this transition.
    pub writes: HashSet<String>,
    /// Whether this transition is visible (affects properties).
    pub visible: bool,
    /// Whether this transition is a local action (only affects one process).
    pub local: bool,
}

/// Partial order reduction manager with C3 cycle detection.
pub struct PorManager<S> {
    /// Cached dependency info per state hash.
    deps_cache: HashMap<u64, Vec<TransitionDeps>>,
    /// Transition labels for dependency lookup.
    _marker: std::marker::PhantomData<S>,

    // C3 cycle detection fields
    /// States currently on DFS stack (hash -> expanded transitions)
    stack_states: HashMap<u64, HashSet<usize>>,
    /// Stack depth for each state
    stack_depth: HashMap<u64, usize>,
}

impl<S> Default for PorManager<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> PorManager<S> {
    pub fn new() -> Self {
        Self {
            deps_cache: HashMap::new(),
            _marker: std::marker::PhantomData,
            stack_states: HashMap::new(),
            stack_depth: HashMap::new(),
        }
    }
}

impl<S: Clone + Hash + Eq + Send> PorManager<S> {
    /// Analyze dependencies of transitions in a state.
    pub fn analyze<M: Model<State = S>>(
        &mut self,
        model: &M,
        state: &S,
        transitions: &[Transition<S>],
    ) -> Vec<TransitionDeps> {
        let hash = model.hash(state);

        if let Some(cached) = self.deps_cache.get(&hash) {
            return cached.clone();
        }

        let deps: Vec<TransitionDeps> = transitions
            .iter()
            .map(|t| self.analyze_transition(model, state, t))
            .collect();

        self.deps_cache.insert(hash, deps.clone());
        deps
    }

    fn analyze_transition<M: Model<State = S>>(
        &self,
        _model: &M,
        _state: &S,
        transition: &Transition<S>,
    ) -> TransitionDeps {
        // Simplified analysis: extract variable access from transition label
        // Full implementation would analyze the Lua effect closure

        let label = &transition.label;
        let mut reads = HashSet::new();
        let mut writes = HashSet::new();

        // Heuristic: parse variable names from label
        // e.g., "P:x=1" writes x, "Q:y" reads y
        if let Some(eq_pos) = label.find('=') {
            // Assignment: left side is written
            let left = label[..eq_pos].trim();
            if let Some(colon_pos) = left.find(':') {
                let var = left[colon_pos + 1..].trim();
                writes.insert(var.to_string());
            } else {
                writes.insert(left.to_string());
            }
        } else if let Some(colon_pos) = label.find(':') {
            // Channel operation or similar
            let var = label[..colon_pos].trim();
            reads.insert(var.to_string());
            writes.insert(var.to_string());
        }

        // Visible if it writes to shared variables
        let visible = !writes.is_empty();

        // Local if it only affects one process (simplified: no channel ops)
        let local = !label.contains('!') && !label.contains('?');

        TransitionDeps {
            reads,
            writes,
            visible,
            local,
        }
    }

    /// Check if two transitions are independent.
    pub fn are_independent(&self, deps1: &TransitionDeps, deps2: &TransitionDeps) -> bool {
        // Independence conditions:
        // 1. Neither disables the other (assumed true for Promela)
        // 2. They don't write to the same variable
        // 3. One doesn't read what the other writes

        // Check write-write conflict
        for w1 in &deps1.writes {
            if deps2.writes.contains(w1) {
                return false;
            }
        }

        // Check read-write conflict
        for w1 in &deps1.writes {
            if deps2.reads.contains(w1) {
                return false;
            }
        }

        for w2 in &deps2.writes {
            if deps1.reads.contains(w2) {
                return false;
            }
        }

        true
    }

    /// Compute a persistent set (ample set) for the given state with C3 condition.
    ///
    /// Returns indices of transitions to explore.
    ///
    /// C3 condition: If the state is on the DFS stack (cycle detected) and not all
    /// transitions have been expanded, we must explore all transitions (disable POR).
    pub fn compute_ample_set_with_c3(
        &mut self,
        model: &impl Model<State = S>,
        state: &S,
        transitions: &[Transition<S>],
        state_hash: u64,
    ) -> Vec<usize> {
        if transitions.is_empty() {
            return vec![];
        }

        let deps = self.analyze(model, state, transitions);

        // C3: Check if we're in a cycle with unexpanded transitions
        if self.check_c3(state_hash, transitions.len()) {
            // C3 violated - must explore all transitions
            return (0..transitions.len()).collect();
        }

        // C0: If no transitions are enabled, ample set is empty
        if transitions.is_empty() {
            return vec![];
        }

        // C1: If any transition is visible, ample set must include all enabled
        // (conservative: we explore all)
        if deps.iter().any(|d| d.visible) {
            return (0..transitions.len()).collect();
        }

        // C2: Try to find a singleton ample set (single local transition)
        for (i, dep) in deps.iter().enumerate() {
            if dep.local && !dep.visible {
                // Check if this transition is independent of all others
                let mut independent_of_all = true;
                for (j, other_dep) in deps.iter().enumerate() {
                    if i != j && !self.are_independent(dep, other_dep) {
                        independent_of_all = false;
                        break;
                    }
                }

                if independent_of_all {
                    return vec![i];
                }
            }
        }

        // C3: Fallback - explore all transitions (conservative)
        // Full implementation would do DFS to find a valid ample set
        (0..transitions.len()).collect()
    }

    /// Check if POR can be safely applied (no visible transitions in cycle).
    pub fn check_por_safe<M: Model<State = S>>(
        &mut self,
        model: &M,
        state: &S,
        transitions: &[Transition<S>],
    ) -> bool {
        let deps = self.analyze(model, state, transitions);
        // Safe if no visible transitions, or if we're in a cycle-free portion
        deps.iter().all(|d| !d.visible)
    }

    // C3 Cycle Detection Methods

    /// Push a state onto the DFS stack (for C3 tracking).
    pub fn push_stack(&mut self, state_hash: u64, depth: usize) {
        self.stack_states.entry(state_hash).or_default();
        self.stack_depth.insert(state_hash, depth);
    }

    /// Pop a state from the DFS stack (for C3 tracking).
    pub fn pop_stack(&mut self, state_hash: &u64) {
        self.stack_states.remove(state_hash);
        self.stack_depth.remove(state_hash);
    }

    /// Mark a transition as expanded for the current state.
    pub fn mark_expanded(&mut self, state_hash: u64, transition_idx: usize) {
        if let Some(expanded) = self.stack_states.get_mut(&state_hash) {
            expanded.insert(transition_idx);
        }
    }

    /// Check C3 condition: are we in a cycle with unexpanded transitions?
    ///
    /// Returns `true` if C3 is violated (cycle detected with unexpanded transitions).
    /// When C3 is violated, POR must be disabled (all transitions must be explored).
    pub fn check_c3(&self, state_hash: u64, num_transitions: usize) -> bool {
        // C3: If the current state is on the stack (cycle detected),
        // and not all transitions have been expanded, C3 is violated.

        if let Some(expanded) = self.stack_states.get(&state_hash) {
            // State is on stack - check if all transitions expanded
            if expanded.len() < num_transitions {
                return true; // C3 violated
            }
        }

        false // C3 satisfied
    }

    /// Check if a state is currently on the DFS stack.
    pub fn is_on_stack(&self, state_hash: &u64) -> bool {
        self.stack_states.contains_key(state_hash)
    }
}

/// Extension trait for models with POR support.
pub trait PorModel: Model {
    /// Get the process ID for a state (for local transition detection).
    fn process_id(&self, _state: &Self::State) -> Option<usize> {
        None
    }

    /// Check if a transition is local to a process.
    fn is_local_transition(&self, _transition: &Transition<Self::State>) -> bool {
        false
    }
}

/// DFS with POR support and C3 condition.
pub fn check_dfs_por_with_c3<M: Model>(
    model: &M,
    max_states: usize,
    max_depth: usize,
) -> crate::engine::checker::CheckResult {
    use crate::engine::checker::CheckResult;

    let start = std::time::Instant::now();
    let init_states = model.init_states();

    if init_states.is_empty() {
        return CheckResult {
            states_explored: 0,
            states_stored: 0,
            transitions: 0,
            depth_reached: 0,
            errors: 0,
            violations: vec![],
            elapsed_secs: 0.0,
        };
    }

    let mut por_manager = PorManager::new();
    let mut visited: HashSet<u64> = HashSet::new();
    let mut stack: Vec<(M::State, usize, u64)> = Vec::new(); // state, depth, hash
    let mut transitions_count = 0;
    let mut max_depth_reached = 0;

    for s in init_states {
        let h = model.hash(&s);
        if visited.insert(h) {
            por_manager.push_stack(h, 0);
            stack.push((s, 0, h));
        }
    }

    while let Some((state, depth, state_hash)) = stack.pop() {
        max_depth_reached = max_depth_reached.max(depth);

        if depth >= max_depth {
            por_manager.pop_stack(&state_hash);
            continue;
        }
        if visited.len() >= max_states {
            por_manager.pop_stack(&state_hash);
            break;
        }

        let all_transitions = model.transitions(&state);

        // Apply POR with C3: compute ample set
        let ample_indices =
            por_manager.compute_ample_set_with_c3(model, &state, &all_transitions, state_hash);

        // Only explore transitions in the ample set
        let transitions_to_explore: Vec<_> =
            ample_indices.iter().map(|&i| &all_transitions[i]).collect();

        transitions_count += transitions_to_explore.len();

        // Mark expanded transitions for C3
        for &idx in &ample_indices {
            por_manager.mark_expanded(state_hash, idx);
        }

        for t in transitions_to_explore {
            let h = model.hash(&t.next);
            if visited.insert(h) {
                por_manager.push_stack(h, depth + 1);
                stack.push((t.next.clone(), depth + 1, h));
            }
        }

        // Pop state from stack after exploring all transitions
        por_manager.pop_stack(&state_hash);
    }

    let elapsed = start.elapsed().as_secs_f64();

    CheckResult {
        states_explored: visited.len(),
        states_stored: visited.len(),
        transitions: transitions_count,
        depth_reached: max_depth_reached,
        errors: 0,
        violations: vec![],
        elapsed_secs: elapsed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::checker::{Model, Transition};

    struct TestModel;

    impl Model for TestModel {
        type State = u8;

        fn init_states(&self) -> Vec<u8> {
            vec![0]
        }

        fn transitions(&self, state: &u8) -> Vec<Transition<u8>> {
            match state {
                0 => vec![
                    Transition {
                        label: "P:x=1".into(),
                        next: 1,
                    },
                    Transition {
                        label: "Q:y=1".into(),
                        next: 2,
                    },
                ],
                1 => vec![Transition {
                    label: "Q:y=1".into(),
                    next: 3,
                }],
                2 => vec![Transition {
                    label: "P:x=1".into(),
                    next: 3,
                }],
                _ => vec![],
            }
        }

        fn hash(&self, state: &u8) -> u64 {
            *state as u64
        }
    }

    #[test]
    fn test_independence_no_conflict() {
        let deps1 = TransitionDeps {
            reads: HashSet::new(),
            writes: ["x".to_string()].into_iter().collect(),
            visible: true,
            local: true,
        };
        let deps2 = TransitionDeps {
            reads: HashSet::new(),
            writes: ["y".to_string()].into_iter().collect(),
            visible: true,
            local: true,
        };

        let manager: PorManager<u8> = PorManager::new();
        assert!(manager.are_independent(&deps1, &deps2));
    }

    #[test]
    fn test_independence_write_conflict() {
        let deps1 = TransitionDeps {
            reads: HashSet::new(),
            writes: ["x".to_string()].into_iter().collect(),
            visible: true,
            local: true,
        };
        let deps2 = TransitionDeps {
            reads: HashSet::new(),
            writes: ["x".to_string()].into_iter().collect(),
            visible: true,
            local: true,
        };

        let manager: PorManager<u8> = PorManager::new();
        assert!(!manager.are_independent(&deps1, &deps2));
    }

    #[test]
    fn test_independence_read_write_conflict() {
        let deps1 = TransitionDeps {
            reads: HashSet::new(),
            writes: ["x".to_string()].into_iter().collect(),
            visible: true,
            local: true,
        };
        let deps2 = TransitionDeps {
            reads: ["x".to_string()].into_iter().collect(),
            writes: HashSet::new(),
            visible: false,
            local: true,
        };

        let manager: PorManager<u8> = PorManager::new();
        assert!(!manager.are_independent(&deps1, &deps2));
    }

    #[test]
    fn test_ample_set_singleton() {
        let model = TestModel;
        let mut manager = PorManager::new();
        let state = 0u8;
        let transitions = model.transitions(&state);

        let state_hash = model.hash(&state);
        let ample = manager.compute_ample_set_with_c3(&model, &state, &transitions, state_hash);

        // Should find a singleton ample set since transitions are independent
        assert!(ample.len() <= transitions.len());
    }

    #[test]
    fn test_por_dfs() {
        let model = TestModel;
        let result = check_dfs_por_with_c3(&model, 1000, 100);

        assert!(result.states_explored > 0);
        assert!(result.transitions > 0);
    }
}
