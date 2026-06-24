//! Büchi automaton construction from LTL formulas.

use std::collections::HashSet;

use crate::property::ltl2ba::error::LtlError;
use crate::property::ltl2ba::formula::LtlFormula;

/// Büchi automaton for LTL verification.
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
    pub conditions: Vec<(String, bool)>,
}

impl BuchiAutomaton {
    /// Create a trivial Büchi automaton with a single state.
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
}

/// Construct Büchi automaton from LTL formula.
///
/// **Supported patterns**:
/// - `[]p` (always): 2-state automaton
/// - `<>p` (eventually): 2-state automaton
/// - `Xp` (next): 2-3 state automaton
/// - `!p`, `p && q`, `p || q`: boolean combinations
///
/// **Unsupported** (returns error):
/// - Nested temporal: `[]<>p`, `<>(p U q)`
/// - Until/Release operators
pub fn to_buchi(formula: &LtlFormula) -> Result<BuchiAutomaton, LtlError> {
    // Check for nested temporal operators
    if let LtlFormula::Always(inner) | LtlFormula::Eventually(inner) | LtlFormula::Next(inner) =
        formula
        && inner.is_temporal()
    {
        return Err(LtlError::nested_temporal(formula.to_string()));
    }

    // Pattern matching for supported formulas
    match formula {
        LtlFormula::True => Ok(trivial_accepting()),
        LtlFormula::False => Ok(trivial_rejecting()),
        LtlFormula::Atom(_) => Ok(atom_to_buchi(formula)),
        LtlFormula::Not(inner) => {
            if inner.is_atomic() {
                Ok(negation_to_buchi(inner))
            } else {
                Err(LtlError::unsupported(
                    "complex negation",
                    Some("Only !p is supported where p is atomic"),
                ))
            }
        }
        LtlFormula::Always(inner) => {
            if inner.is_atomic() {
                Ok(always_to_buchi(inner))
            } else {
                Err(LtlError::unsupported(
                    "complex always",
                    Some("Only []p is supported where p is atomic"),
                ))
            }
        }
        LtlFormula::Eventually(inner) => {
            if inner.is_atomic() {
                Ok(eventually_to_buchi(inner))
            } else {
                Err(LtlError::unsupported(
                    "complex eventually",
                    Some("Only <>p is supported where p is atomic"),
                ))
            }
        }
        LtlFormula::Next(inner) => {
            if inner.is_atomic() {
                Ok(next_to_buchi(inner))
            } else {
                Err(LtlError::unsupported(
                    "complex next",
                    Some("Only Xp is supported where p is atomic"),
                ))
            }
        }
        LtlFormula::And(left, right) => {
            if left.is_atomic() && right.is_atomic() {
                Ok(conjunction_to_buchi(left, right))
            } else {
                Err(LtlError::unsupported(
                    "complex conjunction",
                    Some("Only p && q is supported where p, q are atomic"),
                ))
            }
        }
        LtlFormula::Or(left, right) => {
            if left.is_atomic() && right.is_atomic() {
                Ok(disjunction_to_buchi(left, right))
            } else {
                Err(LtlError::unsupported(
                    "complex disjunction",
                    Some("Only p || q is supported where p, q are atomic"),
                ))
            }
        }
    }
}

/// Create a trivial accepting automaton (1 state, accepting, self-loop).
fn trivial_accepting() -> BuchiAutomaton {
    BuchiAutomaton {
        num_states: 1,
        initial: 0,
        accepting: vec![0].into_iter().collect(),
        transitions: vec![vec![BuchiTransition {
            to: 0,
            conditions: vec![],
        }]],
    }
}

/// Create a trivial rejecting automaton (1 state, non-accepting, self-loop).
fn trivial_rejecting() -> BuchiAutomaton {
    BuchiAutomaton {
        num_states: 1,
        initial: 0,
        accepting: HashSet::new(),
        transitions: vec![vec![BuchiTransition {
            to: 0,
            conditions: vec![],
        }]],
    }
}

/// Convert atomic proposition to Büchi automaton.
fn atom_to_buchi(atom: &LtlFormula) -> BuchiAutomaton {
    let atom_str = match atom {
        LtlFormula::Atom(s) => s.clone(),
        _ => unreachable!(),
    };

    // 1 state, accepting when atom is true
    BuchiAutomaton {
        num_states: 1,
        initial: 0,
        accepting: vec![0].into_iter().collect(),
        transitions: vec![vec![BuchiTransition {
            to: 0,
            conditions: vec![(atom_str, true)],
        }]],
    }
}

/// Convert []p (always) to Büchi automaton.
/// States: s0 (accepting), s1 (rejecting sink)
/// s0 --p--> s0, s0 --!p--> s1, s1 --any--> s1
fn always_to_buchi(atom: &LtlFormula) -> BuchiAutomaton {
    let atom_str = match atom {
        LtlFormula::Atom(s) => s.clone(),
        LtlFormula::True => "true".to_string(),
        LtlFormula::False => "false".to_string(),
        _ => {
            // For complex formulas, use a trivial automaton as fallback
            return trivial_accepting();
        }
    };

    BuchiAutomaton {
        num_states: 2,
        initial: 0,
        accepting: vec![0].into_iter().collect(),
        transitions: vec![
            // s0: accepting, loop on p
            vec![BuchiTransition {
                to: 0,
                conditions: vec![(atom_str.clone(), true)],
            }],
            // s1: rejecting sink
            vec![BuchiTransition {
                to: 1,
                conditions: vec![],
            }],
        ],
    }
}

/// Convert <>p (eventually) to Büchi automaton.
/// States: s0 (non-accepting), s1 (accepting)
/// s0 --!p--> s0, s0 --p--> s1, s1 --any--> s1
fn eventually_to_buchi(atom: &LtlFormula) -> BuchiAutomaton {
    let atom_str = match atom {
        LtlFormula::Atom(s) => s.clone(),
        LtlFormula::True => "true".to_string(),
        LtlFormula::False => "false".to_string(),
        _ => {
            // For complex formulas, use a trivial automaton as fallback
            return trivial_accepting();
        }
    };

    BuchiAutomaton {
        num_states: 2,
        initial: 0,
        accepting: vec![1].into_iter().collect(),
        transitions: vec![
            // s0: wait for p
            vec![
                BuchiTransition {
                    to: 0,
                    conditions: vec![(atom_str.clone(), false)],
                },
                BuchiTransition {
                    to: 1,
                    conditions: vec![(atom_str.clone(), true)],
                },
            ],
            // s1: accepting, stay
            vec![BuchiTransition {
                to: 1,
                conditions: vec![],
            }],
        ],
    }
}

/// Convert Xp (next) to Büchi automaton.
/// States: s0 (initial), s1 (check p), s2 (rejecting)
/// s0 --any--> s1, s1 --p--> s1, s1 --!p--> s2, s2 --any--> s2
fn next_to_buchi(atom: &LtlFormula) -> BuchiAutomaton {
    let atom_str = match atom {
        LtlFormula::Atom(s) => s.clone(),
        _ => unreachable!(),
    };

    BuchiAutomaton {
        num_states: 3,
        initial: 0,
        accepting: vec![1].into_iter().collect(),
        transitions: vec![
            // s0: move to check state
            vec![BuchiTransition {
                to: 1,
                conditions: vec![],
            }],
            // s1: check p
            vec![
                BuchiTransition {
                    to: 1,
                    conditions: vec![(atom_str.clone(), true)],
                },
                BuchiTransition {
                    to: 2,
                    conditions: vec![(atom_str.clone(), false)],
                },
            ],
            // s2: rejecting sink
            vec![BuchiTransition {
                to: 2,
                conditions: vec![],
            }],
        ],
    }
}

/// Convert !p (negation) to Büchi automaton.
fn negation_to_buchi(atom: &LtlFormula) -> BuchiAutomaton {
    let atom_str = match atom {
        LtlFormula::Atom(s) => s.clone(),
        _ => unreachable!(),
    };

    // 1 state, accepting when atom is false
    BuchiAutomaton {
        num_states: 1,
        initial: 0,
        accepting: vec![0].into_iter().collect(),
        transitions: vec![vec![BuchiTransition {
            to: 0,
            conditions: vec![(atom_str, false)],
        }]],
    }
}

/// Convert p && q (conjunction) to Büchi automaton (product construction).
fn conjunction_to_buchi(left: &LtlFormula, right: &LtlFormula) -> BuchiAutomaton {
    let left_auto = atom_to_buchi(left);
    let right_auto = atom_to_buchi(right);
    product_conjunction(&left_auto, &right_auto)
}

/// Convert p || q (disjunction) to Büchi automaton (product construction).
fn disjunction_to_buchi(left: &LtlFormula, right: &LtlFormula) -> BuchiAutomaton {
    let left_auto = atom_to_buchi(left);
    let right_auto = atom_to_buchi(right);
    product_disjunction(&left_auto, &right_auto)
}

/// Product construction for conjunction (intersection of accepting sets).
fn product_conjunction(left: &BuchiAutomaton, right: &BuchiAutomaton) -> BuchiAutomaton {
    let num_states = left.num_states * right.num_states;
    let mut transitions = Vec::with_capacity(num_states);
    let mut accepting = HashSet::new();

    for (l_state, l_trans) in left.transitions.iter().enumerate() {
        for (r_state, r_trans) in right.transitions.iter().enumerate() {
            let prod_state = l_state * right.num_states + r_state;

            // Accepting if both components are accepting
            if left.accepting.contains(&l_state) && right.accepting.contains(&r_state) {
                accepting.insert(prod_state);
            }

            // Product transitions (synchronized)
            let mut prod_trans = Vec::new();
            for l_t in l_trans {
                for r_t in r_trans {
                    let mut conditions = l_t.conditions.clone();
                    conditions.extend(r_t.conditions.clone());
                    let prod_target = l_t.to * right.num_states + r_t.to;
                    prod_trans.push(BuchiTransition {
                        to: prod_target,
                        conditions,
                    });
                }
            }
            transitions.push(prod_trans);
        }
    }

    BuchiAutomaton {
        num_states,
        initial: 0,
        accepting,
        transitions,
    }
}

/// Product construction for disjunction (union of accepting sets).
fn product_disjunction(left: &BuchiAutomaton, right: &BuchiAutomaton) -> BuchiAutomaton {
    let num_states = left.num_states * right.num_states;
    let mut transitions = Vec::with_capacity(num_states);
    let mut accepting = HashSet::new();

    for (l_state, l_trans) in left.transitions.iter().enumerate() {
        for (r_state, r_trans) in right.transitions.iter().enumerate() {
            let prod_state = l_state * right.num_states + r_state;

            // Accepting if either component is accepting
            if left.accepting.contains(&l_state) || right.accepting.contains(&r_state) {
                accepting.insert(prod_state);
            }

            // Product transitions (synchronized)
            let mut prod_trans = Vec::new();
            for l_t in l_trans {
                for r_t in r_trans {
                    let mut conditions = l_t.conditions.clone();
                    conditions.extend(r_t.conditions.clone());
                    let prod_target = l_t.to * right.num_states + r_t.to;
                    prod_trans.push(BuchiTransition {
                        to: prod_target,
                        conditions,
                    });
                }
            }
            transitions.push(prod_trans);
        }
    }

    BuchiAutomaton {
        num_states,
        initial: 0,
        accepting,
        transitions,
    }
}

use std::fmt;

/// Convert formula to string for error messages.
impl fmt::Display for LtlFormula {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LtlFormula::True => write!(fmt, "true"),
            LtlFormula::False => write!(fmt, "false"),
            LtlFormula::Atom(s) => write!(fmt, "{}", s),
            LtlFormula::Not(f) => write!(fmt, "!({})", f),
            LtlFormula::And(f1, f2) => write!(fmt, "({} && {})", f1, f2),
            LtlFormula::Or(f1, f2) => write!(fmt, "({} || {})", f1, f2),
            LtlFormula::Always(f) => write!(fmt, "[]{}", f),
            LtlFormula::Eventually(f) => write!(fmt, "<>{}", f),
            LtlFormula::Next(f) => write!(fmt, "X{}", f),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trivial_accepting() {
        let auto = trivial_accepting();
        assert_eq!(auto.num_states, 1);
        assert!(auto.is_accepting(0));
    }

    #[test]
    fn test_always_pattern() {
        let formula = LtlFormula::Always(Box::new(LtlFormula::Atom("p".to_string())));
        let auto = to_buchi(&formula).unwrap();
        assert_eq!(auto.num_states, 2);
        assert!(auto.is_accepting(0));
        assert!(!auto.is_accepting(1));
    }

    #[test]
    fn test_eventually_pattern() {
        let formula = LtlFormula::Eventually(Box::new(LtlFormula::Atom("p".to_string())));
        let auto = to_buchi(&formula).unwrap();
        assert_eq!(auto.num_states, 2);
        assert!(!auto.is_accepting(0));
        assert!(auto.is_accepting(1));
    }

    #[test]
    fn test_next_pattern() {
        let formula = LtlFormula::Next(Box::new(LtlFormula::Atom("p".to_string())));
        let auto = to_buchi(&formula).unwrap();
        assert_eq!(auto.num_states, 3);
        assert!(auto.is_accepting(1));
    }

    #[test]
    fn test_nested_temporal_error() {
        let formula = LtlFormula::Always(Box::new(LtlFormula::Eventually(Box::new(
            LtlFormula::Atom("p".to_string()),
        ))));
        let result = to_buchi(&formula);
        assert!(matches!(result, Err(LtlError::NestedTemporal { .. })));
    }

    #[test]
    fn test_conjunction() {
        let formula = LtlFormula::And(
            Box::new(LtlFormula::Atom("p".to_string())),
            Box::new(LtlFormula::Atom("q".to_string())),
        );
        let auto = to_buchi(&formula).unwrap();
        assert_eq!(auto.num_states, 1); // Product of two 1-state automata
    }

    #[test]
    fn test_disjunction() {
        let formula = LtlFormula::Or(
            Box::new(LtlFormula::Atom("p".to_string())),
            Box::new(LtlFormula::Atom("q".to_string())),
        );
        let auto = to_buchi(&formula).unwrap();
        assert_eq!(auto.num_states, 1); // Product of two 1-state automata
    }

    #[test]
    fn test_true_false() {
        assert!(to_buchi(&LtlFormula::True).unwrap().is_accepting(0));
        assert!(!to_buchi(&LtlFormula::False).unwrap().is_accepting(0));
    }

    #[test]
    fn test_complex_negation() {
        let formula = LtlFormula::Not(Box::new(LtlFormula::Always(Box::new(LtlFormula::Atom(
            "p".to_string(),
        )))));
        let result = to_buchi(&formula);
        assert!(matches!(result, Err(LtlError::UnsupportedOperator { .. })));
    }

    #[test]
    fn test_complex_always() {
        let formula = LtlFormula::Always(Box::new(LtlFormula::And(
            Box::new(LtlFormula::Atom("p".to_string())),
            Box::new(LtlFormula::Atom("q".to_string())),
        )));
        let result = to_buchi(&formula);
        assert!(matches!(result, Err(LtlError::UnsupportedOperator { .. })));
    }

    #[test]
    fn test_complex_eventually() {
        let formula = LtlFormula::Eventually(Box::new(LtlFormula::And(
            Box::new(LtlFormula::Atom("p".to_string())),
            Box::new(LtlFormula::Atom("q".to_string())),
        )));
        let result = to_buchi(&formula);
        assert!(matches!(result, Err(LtlError::UnsupportedOperator { .. })));
    }

    #[test]
    fn test_atom_pattern() {
        let auto = atom_to_buchi(&LtlFormula::Atom("p".to_string()));
        assert_eq!(auto.num_states, 1);
        assert!(auto.is_accepting(0));
    }

    #[test]
    fn test_trivial_rejecting() {
        let auto = trivial_rejecting();
        assert_eq!(auto.num_states, 1);
        assert!(!auto.is_accepting(0));
    }

    #[test]
    fn test_product_conjunction_disjunction() {
        let left = always_to_buchi(&LtlFormula::Atom("p".to_string()));
        let right = always_to_buchi(&LtlFormula::Atom("q".to_string()));

        let conj = product_conjunction(&left, &right);
        assert_eq!(conj.num_states, 4);

        let disj = product_disjunction(&left, &right);
        assert_eq!(disj.num_states, 4);
    }

    #[test]
    fn test_complex_conjunction() {
        let formula = LtlFormula::And(
            Box::new(LtlFormula::Always(Box::new(LtlFormula::Atom(
                "p".to_string(),
            )))),
            Box::new(LtlFormula::Atom("q".to_string())),
        );
        let result = to_buchi(&formula);
        assert!(matches!(result, Err(LtlError::UnsupportedOperator { .. })));
    }
}
