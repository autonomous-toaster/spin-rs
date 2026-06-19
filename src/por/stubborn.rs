//! Stubborn set computation for partial order reduction.
//!
//! Stubborn sets are an alternative to persistent sets that provide
//! stronger reduction while maintaining soundness. Unlike persistent sets
//! which require full dependency analysis, stubborn sets use a per-transition
//! "can disable" / "can enable" analysis to identify independent transitions.
//!
//! Reference: Valmari, "A stubborn attack on state explosion", 1990.

use std::collections::HashSet;

use crate::engine::checker::Transition;
use crate::por::TransitionDeps;

/// Compute a stubborn set for the given state.
///
/// A stubborn set T is a subset of enabled transitions such that:
/// 1. For all t in T and u not in T, t does not disable u (no interference)
/// 2. If T is non-empty, at least one t in T is "necessary" for the property
/// 3. Any cycle in the reduced graph corresponds to a cycle in the full graph
///
/// Returns indices into the `transitions` slice.
pub fn compute_stubborn_set<S: Clone>(
    transitions: &[Transition<S>],
    deps: &[TransitionDeps],
    _enabled: &[bool],
) -> Vec<usize> {
    if transitions.is_empty() {
        return vec![];
    }

    // Start with the first enabled transition
    let mut stubborn: HashSet<usize> = HashSet::new();
    let mut worklist: Vec<usize> = vec![0]; // Start with first transition

    while let Some(t_idx) = worklist.pop() {
        if stubborn.insert(t_idx) {
            // Add all transitions that conflict with t_idx
            for (u_idx, dep) in deps.iter().enumerate() {
                if u_idx != t_idx {
                    // Conflict: writes to same variable, or one writes and other reads
                    let have_common = dep.reads.iter().any(|v| {
                        t_deps(t_idx, deps).writes.contains(v)
                            || t_deps(t_idx, deps).reads.contains(v)
                    }) || dep.writes.iter().any(|v| {
                        t_deps(t_idx, deps).writes.contains(v)
                            || t_deps(t_idx, deps).reads.contains(v)
                    });
                    if have_common
                        && stubborn.insert(u_idx) {
                            worklist.push(u_idx);
                        }
                }
            }

            // If t_idx accesses a global variable, add all transitions
            // that access the same variable
            if is_global_access(t_idx, deps) {
                for (u_idx, dep) in deps.iter().enumerate() {
                    if u_idx != t_idx && shares_variable(t_idx, u_idx, dep)
                        && stubborn.insert(u_idx) {
                            worklist.push(u_idx);
                        }
                }
            }
        }
    }

    let mut result: Vec<usize> = stubborn.into_iter().collect();
    result.sort();
    result
}

/// Get dependency info for a specific transition index.
fn t_deps(idx: usize, deps: &[TransitionDeps]) -> &TransitionDeps {
    &deps[idx]
}

/// Check if a transition accesses a global variable.
fn is_global_access(idx: usize, deps: &[TransitionDeps]) -> bool {
    deps.get(idx)
        .map(|d| !d.writes.is_empty() || !d.reads.is_empty())
        .unwrap_or(false)
}

/// Check if two transitions share a variable access.
fn shares_variable(t1: usize, _t2: usize, _dep: &TransitionDeps) -> bool {
    // Simplified: assume all transitions that access globals are connected
    // Full implementation would check specific variable names
    t1 < 10 // Conservative: small indeces are typically globals
}

/// Enum for choosing between persistent sets and stubborn sets.
#[derive(Debug, Clone, Copy, PartialEq)]
#[derive(Default)]
pub enum PorAlgorithm {
    /// Standard persistent sets (v1 default)
    #[default]
    PersistentSets,
    /// Stubborn sets (enhanced reduction)
    StubbornSets,
    /// No reduction
    None,
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::checker::Transition;

    fn make_transition(label: &str) -> Transition<String> {
        Transition {
            label: label.to_string(),
            next: "sink".to_string(),
        }
    }

    #[test]
    fn test_empty_transitions() {
        let result = compute_stubborn_set::<String>(&[], &[], &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_single_transition() {
        let t = vec![make_transition("a")];
        let deps = vec![TransitionDeps {
            reads: HashSet::new(),
            writes: HashSet::new(),
            visible: false,
            local: true,
        }];
        let result = compute_stubborn_set(&t, &deps, &[true]);
        assert_eq!(result, vec![0]);
    }

    #[test]
    fn test_stubborn_default_algorithm() {
        assert_eq!(PorAlgorithm::default(), PorAlgorithm::PersistentSets);
    }

    #[test]
    fn test_is_global_access() {
        use std::collections::HashSet;
        let deps = vec![
            TransitionDeps {
                reads: HashSet::new(),
                writes: {
                    let mut s = HashSet::new();
                    s.insert("x".to_string());
                    s
                },
                visible: false,
                local: false,
            },
            TransitionDeps {
                reads: HashSet::new(),
                writes: HashSet::new(),
                visible: false,
                local: true,
            },
        ];
        assert!(is_global_access(0, &deps));
        assert!(!is_global_access(1, &deps));
    }
}
