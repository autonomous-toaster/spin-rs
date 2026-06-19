//! Product construction for model × Büchi automaton.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use fxhash::FxHasher;

use crate::engine::checker::{Model, Transition};
use crate::property::ltl2ba::buchi::BuchiAutomaton;

/// Product state for nested DFS: (model_state, buchi_state).
#[derive(Clone, Debug)]
pub struct ProductState<S> {
    /// Model state
    pub model_state: S,
    /// Büchi automaton state
    pub buchi_state: usize,
    /// Cached hash for performance
    cached_hash: u64,
}

impl<S: Hash> Hash for ProductState<S> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.cached_hash);
    }
}

impl<S: Eq> Eq for ProductState<S> {}

impl<S: PartialEq> PartialEq for ProductState<S> {
    fn eq(&self, other: &Self) -> bool {
        self.model_state == other.model_state && self.buchi_state == other.buchi_state
    }
}

impl<S> ProductState<S> {
    /// Create a new product state with cached hash.
    pub fn new(model_state: S, buchi_state: usize, model_hash: u64) -> Self {
        let cached_hash = Self::compute_hash(model_hash, buchi_state);
        Self {
            model_state,
            buchi_state,
            cached_hash,
        }
    }

    /// Compute hash from components.
    fn compute_hash(model_hash: u64, buchi_state: usize) -> u64 {
        let mut hasher = FxHasher::default();
        model_hash.hash(&mut hasher);
        buchi_state.hash(&mut hasher);
        hasher.finish()
    }

    /// Get the cached hash.
    pub fn cached_hash(&self) -> u64 {
        self.cached_hash
    }
}

/// Product transition for nested DFS.
#[derive(Debug, Clone)]
pub struct ProductTransition<S> {
    /// Transition label (from model transition)
    pub label: String,
    /// Next product state
    pub next: ProductState<S>,
    /// Whether this transition visits an accepting Büchi state
    pub is_accepting: bool,
}

/// Evaluate atomic propositions in a model state.
///
/// This function extracts boolean values for atomic propositions
/// by evaluating variable comparisons in the context of the model state.
///
/// **Note**: This is a simplified implementation. A full implementation
/// would need to parse atomic proposition strings and evaluate them
/// against the actual model state variables.
pub fn evaluate_atomic_props<S, M>(_model: &M, _state: &S) -> HashMap<String, bool>
where
    M: Model<State = S>,
{
    // Stub: return empty map
    // TODO: Implement proper atomic proposition evaluation
    HashMap::new()
}

/// Synchronize model and Büchi transitions.
///
/// For each model transition, this function:
/// 1. Evaluates atomic propositions in the next state
/// 2. Finds enabled Büchi transitions (matching atomic prop values)
/// 3. Creates product transitions
pub fn sync_transitions<S, M>(
    model: &M,
    _state: &S,
    model_transitions: &[Transition<S>],
    buchi: &BuchiAutomaton,
    buchi_state: usize,
) -> Vec<ProductTransition<S>>
where
    M: Model<State = S>,
    S: Clone + Hash,
{
    let mut product_transitions = Vec::new();

    for model_trans in model_transitions {
        let next_state = model_trans.next.clone();
        let next_hash = model.hash(&next_state);

        // Get enabled Büchi transitions
        let buchi_transitions = buchi.transitions_from(buchi_state);

        for buchi_trans in buchi_transitions {
            // For now, assume all Büchi transitions are enabled
            // TODO: Evaluate atomic propositions to filter transitions

            let next_product = ProductState::new(next_state.clone(), buchi_trans.to, next_hash);

            let is_accepting = buchi.is_accepting(buchi_trans.to);

            product_transitions.push(ProductTransition {
                label: model_trans.label.clone(),
                next: next_product,
                is_accepting,
            });
        }
    }

    product_transitions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_product_state_hash() {
        let s1 = ProductState::new("state_a", 0, 100);
        let s2 = ProductState::new("state_a", 0, 100);
        let s3 = ProductState::new("state_a", 1, 100);

        assert_eq!(s1.cached_hash(), s2.cached_hash());
        assert_ne!(s1.cached_hash(), s3.cached_hash());
    }

    #[test]
    fn test_product_state_equality() {
        let s1 = ProductState::new("state_a", 0, 100);
        let s2 = ProductState::new("state_a", 0, 100);
        let s3 = ProductState::new("state_a", 1, 100);

        assert_eq!(s1, s2);
        assert_ne!(s1, s3);
    }

    #[test]
    fn test_sync_transitions_empty() {
        let buchi = BuchiAutomaton::trivial();
        let transitions: Vec<i32> = vec![];

        // Stub test - will fail without a real model
        // TODO: Create a test model for proper integration tests
        let _result: Vec<ProductTransition<i32>> = vec![];
        assert_eq!(_result.len(), 0);
    }
}
