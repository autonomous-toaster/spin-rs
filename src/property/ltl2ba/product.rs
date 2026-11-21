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
/// For LuaModel, this extracts boolean values by parsing the state blob
/// and evaluating variable comparisons like "x == 0", "flag", etc.
///
/// **Note**: This is a simplified implementation that parses the JSON-like
/// state blob format used by LuaModel.
pub fn evaluate_atomic_props<S, M>(model: &M, state: &S) -> HashMap<String, bool>
where
    M: Model<State = S>,
{
    // Resolve through the Model trait. This used to reinterpret the state pointer as a
    // StateBlob, which is undefined behaviour for every other state type (for example a
    // u64 in the unit tests) and crashed once the caller evaluated a successor state.
    let mut props = model.atomic_props(state).unwrap_or_default();

    // Always add "true" as true and "false" as false for Büchi conditions
    props.insert("true".to_string(), true);
    props.insert("false".to_string(), false);

    props
}

/// Synchronize model and Büchi transitions.
///
/// For each model transition, this function:
/// 1. Evaluates atomic propositions in the next state
/// 2. Finds enabled Büchi transitions (matching atomic prop values)
/// 3. Creates product transitions
pub fn sync_transitions<S, M>(
    model: &M,
    state: &S,
    model_transitions: &[Transition<S>],
    buchi: &BuchiAutomaton,
    buchi_state: usize,
) -> Vec<ProductTransition<S>>
where
    M: Model<State = S>,
    S: Clone + Hash,
{
    let mut product_transitions = Vec::new();

    let _ = state;

    for model_trans in model_transitions {
        let next_state = model_trans.next.clone();
        let next_hash = model.hash(&next_state);

        // Evaluate atomic propositions in the SUCCESSOR state. Reading them from the
        // source state made each [] obligation behave like X: the automaton advanced on
        // the current state's values while stepping to the next model state.
        let props = evaluate_atomic_props(model, &next_state);

        // Get enabled Büchi transitions
        let buchi_transitions = buchi.transitions_from(buchi_state);

        for buchi_trans in buchi_transitions {
            // Check if all conditions are satisfied.
            //
            // An absent atom is deliberately treated as unsatisfied rather than as
            // "false": callers validate every atom against the initial state before the
            // search (see PropertyChecker::check_liveness), so reaching a missing atom
            // here means the state space diverged from that validation and the safe
            // action is to leave the Büchi transition disabled rather than to assert a
            // value we did not observe.
            let all_conditions_met = buchi_trans.conditions.iter().all(|(prop, must_be_true)| {
                match props.get(prop.as_str()) {
                    Some(prop_val) => prop_val == must_be_true,
                    None => false,
                }
            });

            if !all_conditions_met {
                continue;
            }

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
        let _buchi = BuchiAutomaton::trivial();
        let _transitions: Vec<i32> = vec![];

        // Stub test - will fail without a real model
        // TODO: Create a test model for proper integration tests
        let _result: Vec<ProductTransition<i32>> = vec![];
        assert_eq!(_result.len(), 0);
    }
}
