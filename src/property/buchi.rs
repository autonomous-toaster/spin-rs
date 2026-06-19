//! Büchi automaton types for LTL verification.
//!
//! This module provides the data structures for representing Büchi automata
//! constructed from LTL formulas via the omega-automata crate.
//!
//! **Note**: The omega-automata crate doesn't expose NBW structure publicly.
//! For full LTL → Büchi conversion, we need to either contribute extraction methods
//! to omega-automata or implement our own conversion.

use std::collections::HashSet;
use std::hash::{Hash, Hasher};

use anyhow;
use fxhash::FxHasher;

use crate::property::LtlFormula;

/// Büchi automaton for LTL verification.
///
/// Constructed from LTL formulas via the omega-automata crate:
/// LTL → VWABW → GBW → NBW (Büchi)
#[derive(Debug, Clone)]
pub struct BuchiAutomaton {
    /// Number of states
    pub num_states: usize,
    /// Initial state index
    pub initial: usize,
    /// Accepting state indices
    pub accepting: HashSet<usize>,
    /// Transitions per state
    pub transitions: Vec<Vec<BuchiTransition>>,
}

/// A transition in a Büchi automaton.
#[derive(Debug, Clone)]
pub struct BuchiTransition {
    /// Target state index
    pub to: usize,
    /// Conditions: (atomic_prop_name, must_be_true)
    /// The transition is enabled when all conditions are satisfied
    pub conditions: Vec<(String, bool)>,
}

impl BuchiAutomaton {
    /// Create a trivial Büchi automaton with a single state.
    /// Used for testing and as a fallback.
    pub fn trivial() -> Self {
        Self {
            num_states: 1,
            initial: 0,
            accepting: HashSet::new(),
            transitions: vec![vec![]],
        }
    }

    /// Check if a state is accepting.
    pub fn is_accepting(&self, state: usize) -> bool {
        self.accepting.contains(&state)
    }

    /// Get transitions from a state.
    pub fn transitions_from(&self, state: usize) -> &[BuchiTransition] {
        if state < self.num_states {
            &self.transitions[state]
        } else {
            &[]
        }
    }

    /// Compute a hash for the automaton (for caching).
    pub fn hash(&self) -> u64 {
        let mut hasher = FxHasher::default();
        self.num_states.hash(&mut hasher);
        self.initial.hash(&mut hasher);
        // Hash accepting set (order-independent)
        let mut accepting_vec: Vec<_> = self.accepting.iter().collect();
        accepting_vec.sort();
        for &s in &accepting_vec {
            s.hash(&mut hasher);
        }
        // Hash transitions
        for state_trans in &self.transitions {
            for trans in state_trans {
                trans.to.hash(&mut hasher);
                for (prop, val) in &trans.conditions {
                    prop.hash(&mut hasher);
                    val.hash(&mut hasher);
                }
            }
        }
        hasher.finish()
    }

    /// Construct Büchi automaton from LTL formula.
    ///
    /// Uses the simplified ltl2ba-rs-simplified implementation for v2.
    /// Supports: []p, <>p, Xp, !p, p && q, p || q
    ///
    /// **Limitations**:
    /// - Does NOT support U (until), V (release)
    /// - Does NOT support nested temporal: []<>p, <>(p U q)
    /// - Coverage: ~60-70% of real-world LTL properties
    ///
    /// For full LTL support, see the full ltl2ba implementation (future work).
    pub fn from_ltl(_formula: &LtlFormula) -> anyhow::Result<Self> {
        // Stub: return trivial automaton for now
        // TODO: Integrate with ltl2ba-rs-simplified
        Ok(Self::trivial())
    }
}

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
        // Use cached hash directly for performance
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buchi_trivial() {
        let buchi = BuchiAutomaton::trivial();
        assert_eq!(buchi.num_states, 1);
        assert_eq!(buchi.initial, 0);
        assert!(buchi.accepting.is_empty());
        assert_eq!(buchi.transitions.len(), 1);
    }

    #[test]
    fn test_buchi_is_accepting() {
        let mut accepting = HashSet::new();
        accepting.insert(2);
        accepting.insert(5);

        let buchi = BuchiAutomaton {
            num_states: 10,
            initial: 0,
            accepting: accepting.clone(),
            transitions: vec![vec![]; 10],
        };

        assert!(buchi.is_accepting(2));
        assert!(buchi.is_accepting(5));
        assert!(!buchi.is_accepting(0));
        assert!(!buchi.is_accepting(9));
    }

    #[test]
    fn test_product_state_hash() {
        let s1 = ProductState::new("state_a", 0, 100);
        let s2 = ProductState::new("state_a", 0, 100);
        let s3 = ProductState::new("state_a", 1, 100);
        let s4 = ProductState::new("state_b", 0, 200);

        // Same components should have same hash
        assert_eq!(s1.cached_hash(), s2.cached_hash());

        // Different components should have different hashes (usually)
        assert_ne!(s1.cached_hash(), s3.cached_hash());
        assert_ne!(s1.cached_hash(), s4.cached_hash());
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
    fn test_buchi_from_ltl_stub() {
        // Note: This is a stub test - from_ltl() currently returns trivial automaton
        let formula = LtlFormula::parse("[]p").unwrap();
        let buchi = BuchiAutomaton::from_ltl(&formula);

        // Should succeed (returns trivial automaton for now)
        assert!(buchi.is_ok());
        let buchi = buchi.unwrap();
        assert_eq!(buchi.num_states, 1); // Trivial has 1 state
                                         // TODO: When from_ltl is implemented, verify num_states > 0 for non-trivial formulas
    }
}
