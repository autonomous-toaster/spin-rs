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
            // s0: accepting, loop on p; transition to sink on !p
            vec![
                BuchiTransition {
                    to: 0,
                    conditions: vec![(atom_str.clone(), true)],
                },
                BuchiTransition {
                    to: 1,
                    conditions: vec![(atom_str.clone(), false)],
                },
            ],
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

#[cfg(test)]
mod tests;

/// Construct the Büchi automaton for the NEGATION of a formula.
///
/// A property is violated when a run satisfies its negation, so the product search must
/// look for accepting cycles of the negated automaton. Building the automaton for the
/// formula itself and looking for accepting cycles reports the opposite verdict: it flags
/// satisfied properties and misses violated ones.
///
/// Only the deterministic shapes produced by the builders below are complemented, where
/// the complement is obtained by swapping the accepting set and the condition polarity.
/// Anything else returns an error rather than a guess.
pub fn to_buchi_not(formula: &LtlFormula) -> Result<BuchiAutomaton, LtlError> {
    match formula {
        // !true is unsatisfiable; !false holds everywhere.
        LtlFormula::True => Ok(trivial_rejecting()),
        LtlFormula::False => Ok(trivial_accepting()),

        // A propositional property is violated exactly when the atom is false here.
        LtlFormula::Atom(name) => Ok(single_state_with(name.clone(), false)),
        LtlFormula::Not(inner) => match inner.as_ref() {
            LtlFormula::Atom(name) => Ok(single_state_with(name.clone(), true)),
            _ => Err(LtlError::unsupported(
                "negation of a non-atomic proposition",
                Some("Only !p is supported where p is atomic"),
            )),
        },

        // ![](a -> <>b)  is  <>(a && []!b): the obligation is broken when a finally
        // happens and b never does afterwards. This is the shape every veriplan
        // conditional constraint has, so it is worth supporting explicitly.
        LtlFormula::Always(inner) => {
            if let Some(not_buchi) = complement_implies_eventually(inner) {
                return Ok(not_buchi);
            }
            let (name, must_be_true) = atom_with_polarity(inner, "always")?;
            Ok(eventually_holds(name, !must_be_true))
        }
        // !<>p  is  []!p
        LtlFormula::Eventually(inner) => {
            let (name, must_be_true) = atom_with_polarity(inner, "eventually")?;
            Ok(always_holds(name, !must_be_true))
        }
        // !Xp  is  X!p
        LtlFormula::Next(inner) => {
            let (name, must_be_true) = atom_with_polarity(inner, "next")?;
            Ok(next_holds(name, !must_be_true))
        }

        // Every spelling of the unwrapped form declines - `a == 1 ->`, `a == 0 ->`, and one whose
        // antecedent becomes true only later - measured three ways here and independently by a
        // consumer. An earlier release note claimed one of the three decided; that was a grep which
        // matched `errors: 0` and not the "Inconclusive" line under it, and there is no such case.
        // Nothing about this decline depends on anything outside the property.
        //
        // `a -> <>b` written without an outer box. Measured with pan rather than reasoned about:
        // for a model where `a` only becomes true later, pan accepts both the unboxed form and the
        // boxed one differently - unboxed 0, boxed 1 - so pan evaluates an unboxed formula at the
        // *initial state*, and the negation is `a at position 0 and never b`, which is not the
        // automaton this matcher builds for `[] ( a -> <> b )`. Supporting it needs a
        // start-state-sensitive automaton, so it declines by name and says what to write instead.
        LtlFormula::And(_, _) | LtlFormula::Or(_, _) => Err(LtlError::unsupported(
            "a -> <> b without an outer []",
            Some(
                "pan evaluates an unwrapped formula at the initial state, so this is not the same property as [] ( a -> <> b ): measured on a model where a only becomes true later, the unwrapped form is pan 0 and the boxed one pan 1. Write the boxed form if you mean the response property, and know that it is the stronger of the two",
            ),
        )),
    }
}

/// Extract the atom name and the polarity the property requires of it.
fn atom_with_polarity(atom: &LtlFormula, ctx: &'static str) -> Result<(String, bool), LtlError> {
    match atom {
        LtlFormula::Atom(name) => Ok((name.clone(), true)),
        LtlFormula::True => Ok(("true".to_string(), true)),
        LtlFormula::False => Ok(("false".to_string(), true)),
        LtlFormula::Not(inner) => match inner.as_ref() {
            LtlFormula::Atom(name) => Ok((name.clone(), false)),
            LtlFormula::True => Ok(("true".to_string(), false)),
            LtlFormula::False => Ok(("false".to_string(), false)),
            _ => Err(LtlError::unsupported(
                "complex atom under a temporal operator",
                Some(match ctx {
                    "always" => "Only []p is supported where p is atomic",
                    "eventually" => "Only <>p is supported where p is atomic",
                    _ => "Only Xp is supported where p is atomic",
                }),
            )),
        },
        _ => Err(LtlError::unsupported(
            "complex atom under a temporal operator",
            Some("Only p or !p is supported where p is atomic"),
        )),
    }
}

/// One accepting state looping while `name` equals `value`.
fn single_state_with(name: String, value: bool) -> BuchiAutomaton {
    BuchiAutomaton {
        num_states: 1,
        initial: 0,
        accepting: vec![0].into_iter().collect(),
        transitions: vec![vec![BuchiTransition {
            to: 0,
            conditions: vec![(name, value)],
        }]],
    }
}

/// s0 --match--> s0 (accepting); s0 --!match--> s1 (rejecting sink); s1 any-> s1.
fn always_holds(name: String, value: bool) -> BuchiAutomaton {
    BuchiAutomaton {
        num_states: 2,
        initial: 0,
        accepting: vec![0].into_iter().collect(),
        transitions: vec![
            vec![
                BuchiTransition {
                    to: 0,
                    conditions: vec![(name.clone(), value)],
                },
                BuchiTransition {
                    to: 1,
                    conditions: vec![(name, !value)],
                },
            ],
            vec![BuchiTransition {
                to: 1,
                conditions: vec![],
            }],
        ],
    }
}

/// s0 --!match--> s0; s0 --match--> s1 (accepting); s1 any-> s1.
fn eventually_holds(name: String, value: bool) -> BuchiAutomaton {
    BuchiAutomaton {
        num_states: 2,
        initial: 0,
        accepting: vec![1].into_iter().collect(),
        transitions: vec![
            vec![
                BuchiTransition {
                    to: 0,
                    conditions: vec![(name.clone(), !value)],
                },
                BuchiTransition {
                    to: 1,
                    conditions: vec![(name, value)],
                },
            ],
            vec![BuchiTransition {
                to: 1,
                conditions: vec![],
            }],
        ],
    }
}

/// s0 any-> s1; s1 --match--> s1 (accepting); s1 --!match--> s2 (rejecting); s2 any-> s2.
fn next_holds(name: String, value: bool) -> BuchiAutomaton {
    BuchiAutomaton {
        num_states: 3,
        initial: 0,
        accepting: vec![1].into_iter().collect(),
        transitions: vec![
            vec![BuchiTransition {
                to: 1,
                conditions: vec![],
            }],
            vec![
                BuchiTransition {
                    to: 1,
                    conditions: vec![(name.clone(), value)],
                },
                BuchiTransition {
                    to: 2,
                    conditions: vec![(name, !value)],
                },
            ],
            vec![BuchiTransition {
                to: 2,
                conditions: vec![],
            }],
        ],
    }
}

/// Complement of `[] (lhs -> <> rhs)`, i.e. `<> (lhs && [] !rhs)`.
///
/// The converted formula arrives as `Or(Not(lhs), Eventually(rhs))` because implies is
/// rewritten to a disjunction. Returns `None` for any other shape so the caller can report
/// it as unsupported rather than guess.
///
/// States: S0 waits for the antecedent, S1 holds after it fired and until the consequent
/// obliges again. Staying in S1 forever means the antecedent happened and the consequent
/// never did, which is exactly the violation, so S1 is accepting.
fn complement_implies_eventually(inner: &LtlFormula) -> Option<BuchiAutomaton> {
    let LtlFormula::Or(left, right) = inner else {
        return None;
    };
    let LtlFormula::Not(lhs) = left.as_ref() else {
        return None;
    };
    let LtlFormula::Eventually(rhs) = right.as_ref() else {
        return None;
    };

    // The automaton must commit to ONE point where the antecedent fires and from which the
    // consequent never arrives. Letting it return to the waiting state on the consequent
    // allowed it to re-guess, so a run where the consequent keeps arriving was accepted as
    // a violation -- a false positive on a property that holds. A dead state on the
    // consequent removes the re-guess.
    //
    // The guards are propositional, not necessarily single literals: the consequent may be
    // any boolean combination of atoms (a contradiction such as (x && !x) is a legitimate
    // way to write an unsatisfiable obligation).
    let dnf_enter = literals_dnf(lhs, false)?;
    let dnf_wait = literals_dnf(lhs, true)?;
    let dnf_stay = literals_dnf(rhs, true)?;
    let dnf_stop = literals_dnf(rhs, false)?;

    let mut s0 = Vec::new();
    for cube in dnf_enter {
        s0.push(BuchiTransition {
            to: 1,
            conditions: cube,
        });
    }
    for cube in dnf_wait {
        s0.push(BuchiTransition {
            to: 0,
            conditions: cube,
        });
    }

    let mut s1 = Vec::new();
    // Stay accepting while the consequent has not arrived.
    for cube in dnf_stay {
        s1.push(BuchiTransition {
            to: 1,
            conditions: cube,
        });
    }
    // The consequent arrived, so this guess cannot witness a violation.
    for cube in dnf_stop {
        s1.push(BuchiTransition {
            to: 2,
            conditions: cube,
        });
    }

    Some(BuchiAutomaton {
        num_states: 3,
        initial: 0,
        accepting: vec![1].into_iter().collect(),
        transitions: vec![
            s0,
            s1,
            vec![BuchiTransition {
                to: 2,
                conditions: vec![],
            }],
        ],
    })
}

/// Disjunctive normal form of a propositional formula, as a list of cubes.
///
/// Each cube is a conjunction of literals. `negate` asks for the DNF of the negated formula.
/// Returns `None` for anything temporal, so unsupported shapes are reported rather than
/// guessed. Empty cubes are dropped because they can never be satisfied.
fn literals_dnf(f: &LtlFormula, negate: bool) -> Option<Vec<Vec<(String, bool)>>> {
    match f {
        LtlFormula::Atom(name) => Some(vec![vec![(name.clone(), !negate)]]),
        LtlFormula::True => Some(if negate { vec![] } else { vec![vec![]] }),
        LtlFormula::False => Some(if negate { vec![vec![]] } else { vec![] }),
        LtlFormula::Not(inner) => literals_dnf(inner, !negate),
        LtlFormula::And(a, b) => {
            if negate {
                // !(a && b) = !a || !b
                let mut out = literals_dnf(a, true)?;
                out.extend(literals_dnf(b, true)?);
                Some(out)
            } else {
                Some(cross(literals_dnf(a, false)?, literals_dnf(b, false)?))
            }
        }
        LtlFormula::Or(a, b) => {
            if negate {
                // !(a || b) = !a && !b
                Some(cross(literals_dnf(a, true)?, literals_dnf(b, true)?))
            } else {
                let mut out = literals_dnf(a, false)?;
                out.extend(literals_dnf(b, false)?);
                Some(out)
            }
        }
        LtlFormula::Always(_) | LtlFormula::Eventually(_) | LtlFormula::Next(_) => None,
    }
}

/// Combine two DNFs by conjunction, dropping cubes that demand both polarities of an atom.
fn cross(
    left: Vec<Vec<(String, bool)>>,
    right: Vec<Vec<(String, bool)>>,
) -> Vec<Vec<(String, bool)>> {
    let mut out = Vec::new();
    for l in &left {
        for r in &right {
            let mut cube = l.clone();
            cube.extend(r.clone());
            if !cube_is_contradictory(&cube) {
                out.push(cube);
            }
        }
    }
    out
}

fn cube_is_contradictory(cube: &[(String, bool)]) -> bool {
    cube.iter().enumerate().any(|(i, (name, val))| {
        cube[i + 1..]
            .iter()
            .any(|(other, other_val)| other == name && other_val != val)
    })
}
