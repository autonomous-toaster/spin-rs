//! Nested DFS for accepting cycle detection in LTL verification.

use std::collections::HashSet;
use std::hash::Hash;
use std::marker::PhantomData;

use crate::engine::checker::{Model, Transition, Violation};
use crate::property::ltl2ba::buchi::BuchiAutomaton;
use crate::property::ltl2ba::product::{ProductState, ProductTransition};

/// Nested DFS for LTL verification.
///
/// Implements the nested DFS algorithm for detecting accepting cycles
/// in the product space (model × Büchi automaton).
///
/// **Algorithm** (standard Spin nested DFS):
/// 1. Outer DFS (dfs1) explores the product space
/// 2. When backtracking from an accepting state, inner DFS (dfs2) starts
/// 3. Inner DFS searches for a cycle back to the accepting state
/// 4. If inner DFS finds a state already on its stack, an accepting cycle exists
pub struct NestedDFS<S> {
    /// Outer DFS visited set
    visited1: HashSet<u64>,
    /// Inner DFS visited set
    visited2: HashSet<u64>,
    /// Outer DFS stack (for cycle detection in dfs2 seed)
    stack1: Vec<u64>,
    /// Inner DFS stack (for cycle detection)
    stack2: Vec<u64>,
    /// Transition labels (for error trail)
    trail: Vec<String>,
    /// Maximum search depth (optional)
    max_depth: Option<usize>,
    /// Phantom data for state type
    _phantom: PhantomData<S>,
}

impl<S> NestedDFS<S>
where
    S: Clone + Hash + Eq + Send + 'static,
{
    /// Create a new NestedDFS instance.
    pub fn new() -> Self {
        Self {
            visited1: HashSet::new(),
            visited2: HashSet::new(),
            stack1: Vec::new(),
            stack2: Vec::new(),
            trail: Vec::new(),
            max_depth: None,
            _phantom: PhantomData,
        }
    }

    /// Set maximum search depth.
    pub fn with_max_depth(mut self, max_depth: usize) -> Self {
        self.max_depth = Some(max_depth);
        self
    }

    /// Run nested DFS to check for liveness violations.
    ///
    /// Returns `Some(Violation)` if an accepting cycle is found,
    /// `None` if no violations are detected.
    pub fn check<M>(
        &mut self,
        model: &M,
        buchi: &BuchiAutomaton,
        init_product: ProductState<S>,
    ) -> Option<Violation>
    where
        M: Model<State = S>,
    {
        let init_hash = init_product.cached_hash();

        if let Some(violation) = self.dfs1(model, buchi, init_product, init_hash, 0) {
            return Some(violation);
        }

        None
    }

    /// Outer DFS: explore product space.
    /// When backtracking from an accepting state, start inner DFS (dfs2).
    fn dfs1<M>(
        &mut self,
        model: &M,
        buchi: &BuchiAutomaton,
        product: ProductState<S>,
        hash: u64,
        depth: usize,
    ) -> Option<Violation>
    where
        M: Model<State = S>,
    {
        // Check depth limit
        if let Some(max) = self.max_depth
            && depth >= max
        {
            return None;
        }

        self.visited1.insert(hash);
        self.stack1.push(hash);

        // Get model transitions
        let model_transitions = model.transitions(&product.model_state);

        // Synchronize with Büchi transitions
        let product_transitions = self.sync_with_buchi(
            model,
            &product.model_state,
            &model_transitions,
            buchi,
            product.buchi_state,
        );

        for prod_trans in product_transitions {
            let next_hash = prod_trans.next.cached_hash();

            if !self.visited1.contains(&next_hash) {
                // Explore unvisited state
                self.trail.push(prod_trans.label.clone());
                if let Some(violation) =
                    self.dfs1(model, buchi, prod_trans.next, next_hash, depth + 1)
                {
                    return Some(violation);
                }
                self.trail.pop();
            }
        }

        // After exploring all successors, if this is an accepting state,
        // start inner DFS to search for a cycle
        if buchi.is_accepting(product.buchi_state)
            && let Some(violation) = self.dfs2(model, buchi, product, hash)
        {
            return Some(violation);
        }

        self.stack1.pop();
        None
    }

    /// Inner DFS: search for cycle back to accepting state.
    /// Uses stack2 to detect cycles (a state already on stack2 means a cycle).
    fn dfs2<M>(
        &mut self,
        model: &M,
        buchi: &BuchiAutomaton,
        product: ProductState<S>,
        hash: u64,
    ) -> Option<Violation>
    where
        M: Model<State = S>,
    {
        if self.visited2.contains(&hash) {
            return None; // Already explored by inner DFS
        }

        self.visited2.insert(hash);
        self.stack2.push(hash);

        // Get model transitions
        let model_transitions = model.transitions(&product.model_state);

        // Synchronize with Büchi transitions
        let product_transitions = self.sync_with_buchi(
            model,
            &product.model_state,
            &model_transitions,
            buchi,
            product.buchi_state,
        );

        for prod_trans in product_transitions {
            let next_hash = prod_trans.next.cached_hash();

            if self.stack2.contains(&next_hash) {
                // Cycle detected: state already on inner DFS stack
                return Some(Violation {
                    property_name: "LTL".to_string(),
                    trail: self.trail.clone(),
                    description: "Accepting cycle detected (liveness violation)".to_string(),
                });
            }

            if !self.visited2.contains(&next_hash)
                && let Some(violation) = self.dfs2(model, buchi, prod_trans.next, next_hash)
            {
                return Some(violation);
            }
        }

        self.stack2.pop();
        None
    }

    /// Synchronize model transitions with Büchi transitions.
    fn sync_with_buchi<M>(
        &self,
        model: &M,
        state: &S,
        model_transitions: &[Transition<S>],
        buchi: &BuchiAutomaton,
        buchi_state: usize,
    ) -> Vec<ProductTransition<S>>
    where
        M: Model<State = S>,
    {
        use crate::property::ltl2ba::product::sync_transitions;
        sync_transitions(model, state, model_transitions, buchi, buchi_state)
    }
}

impl<S> Default for NestedDFS<S>
where
    S: Clone + Hash + Eq + Send + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nested_dfs_new() {
        let dfs: NestedDFS<i32> = NestedDFS::new();
        assert!(dfs.visited1.is_empty());
        assert!(dfs.visited2.is_empty());
        assert!(dfs.stack1.is_empty());
        assert!(dfs.trail.is_empty());
        assert!(dfs.max_depth.is_none());
    }

    #[test]
    fn test_nested_dfs_with_max_depth() {
        let dfs: NestedDFS<i32> = NestedDFS::new().with_max_depth(100);
        assert_eq!(dfs.max_depth, Some(100));
    }

    // Simple model for testing nested DFS
    struct TestModel;
    impl crate::engine::checker::Model for TestModel {
        type State = u64;
        fn init_states(&self) -> Vec<u64> {
            vec![0]
        }
        fn transitions(&self, state: &u64) -> Vec<crate::engine::checker::Transition<u64>> {
            match state {
                0 => vec![
                    crate::engine::checker::Transition {
                        label: "a".into(),
                        next: 1,
                    },
                    crate::engine::checker::Transition {
                        label: "b".into(),
                        next: 2,
                    },
                ],
                1 => vec![crate::engine::checker::Transition {
                    label: "c".into(),
                    next: 3,
                }],
                _ => vec![],
            }
        }
        fn hash(&self, state: &u64) -> u64 {
            *state
        }
    }

    #[test]
    fn test_nested_dfs_check_no_violation() {
        // Create a Büchi automaton that accepts []p (always p)
        let mut accepting = std::collections::HashSet::new();
        accepting.insert(0);
        let buchi = crate::property::ltl2ba::buchi::BuchiAutomaton {
            num_states: 2,
            initial: 0,
            accepting,
            transitions: vec![
                vec![crate::property::ltl2ba::buchi::BuchiTransition {
                    to: 0,
                    conditions: vec![("p".to_string(), true)],
                }],
                vec![crate::property::ltl2ba::buchi::BuchiTransition {
                    to: 1,
                    conditions: vec![],
                }],
            ],
        };
        let model = TestModel;
        let init_product =
            crate::property::ltl2ba::product::ProductState::new(0u64, buchi.initial, 0);
        let mut dfs = NestedDFS::new().with_max_depth(10);
        let violation = dfs.check(&model, &buchi, init_product);
        // Model has no accepting cycle, no violation expected
        assert!(violation.is_none());
    }
}
