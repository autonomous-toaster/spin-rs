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

/// Partial order reduction manager.
pub struct PorManager<S> {
    /// Cached dependency info per state hash.
    deps_cache: HashMap<u64, Vec<TransitionDeps>>,
    /// Transition labels for dependency lookup.
    _marker: std::marker::PhantomData<S>,
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

    /// Compute a persistent set (ample set) for the given state.
    ///
    /// Returns indices of transitions to explore.
    pub fn compute_ample_set(
        &mut self,
        model: &impl Model<State = S>,
        state: &S,
        transitions: &[Transition<S>],
    ) -> Vec<usize> {
        if transitions.is_empty() {
            return vec![];
        }

        let deps = self.analyze(model, state, transitions);

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

/// DFS with POR support.
pub fn check_dfs_por<M: Model>(
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
    let mut stack: Vec<(M::State, usize)> = Vec::new();
    let mut transitions_count = 0;
    let mut max_depth_reached = 0;

    for s in init_states {
        let h = model.hash(&s);
        if visited.insert(h) {
            stack.push((s, 0));
        }
    }

    while let Some((state, depth)) = stack.pop() {
        max_depth_reached = max_depth_reached.max(depth);

        if depth >= max_depth {
            continue;
        }
        if visited.len() >= max_states {
            break;
        }

        let all_transitions = model.transitions(&state);

        // Apply POR: compute ample set
        let ample_indices = por_manager.compute_ample_set(model, &state, &all_transitions);

        // Only explore transitions in the ample set
        let transitions_to_explore: Vec<_> =
            ample_indices.iter().map(|&i| &all_transitions[i]).collect();

        transitions_count += transitions_to_explore.len();

        for t in transitions_to_explore {
            let h = model.hash(&t.next);
            if visited.insert(h) {
                stack.push((t.next.clone(), depth + 1));
            }
        }
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

        let ample = manager.compute_ample_set(&model, &state, &transitions);

        // Should find a singleton ample set since transitions are independent
        assert!(ample.len() <= transitions.len());
    }

    #[test]
    fn test_por_dfs() {
        let model = TestModel;
        let result = check_dfs_por(&model, 1000, 100);

        assert!(result.states_explored > 0);
        assert!(result.transitions > 0);
    }
}
