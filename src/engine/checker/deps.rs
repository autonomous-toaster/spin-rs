use std::collections::BTreeSet;

/// What one transition reads and writes, in the model's own vocabulary.
///
/// The fields name *state variables*, not Promela identifiers, because that is what collides:
/// two proctypes each with a local x hold p1_x and p2_x and are independent, while two
/// transitions touching the global x are not.
///
/// # The safe default
///
/// TransitionDeps::default() is **opaque**: a transition that says nothing about itself is
/// assumed to conflict with everything. The opposite default - an empty read/write set - would
/// claim independence from every other transition, turning a missing annotation into a silently
/// reduced search. Over-approximation costs reduction; under-approximation loses violations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionDeps {
    /// Variables the guard evaluates.
    pub reads: Vec<String>,
    /// Variables the effect assigns.
    pub writes: Vec<String>,
    /// True when the transition writes only its own process's bookkeeping (_step_, _done_,
    /// _pc_), so it cannot change any property's valuation.
    pub internal: bool,
    /// True when the emitter could not state the dependencies precisely.
    pub opaque: bool,
    /// The process this transition belongs to, as the generator names it.
    ///
    /// pan's `priority` restricts which processes may move, so the engine has to know which
    /// process an enabled transition came from; the emitter already wrote this field for its own
    /// bookkeeping.
    pub owner: String,
}

impl Default for TransitionDeps {
    fn default() -> Self {
        Self {
            reads: Vec::new(),
            writes: Vec::new(),
            owner: String::new(),
            internal: false,
            opaque: true,
        }
    }
}

impl TransitionDeps {
    /// A transition that declares what it touches.
    pub fn new(reads: &[&str], writes: &[&str], internal: bool) -> Self {
        Self {
            reads: reads.iter().map(|s| s.to_string()).collect(),
            writes: writes.iter().map(|s| s.to_string()).collect(),
            internal,
            opaque: false,
            owner: String::new(),
        }
    }

    /// Whether this transition can change the valuation of a proposition drawn from visible.
    ///
    /// Only writes matter: a guard that reads a visible variable does not change the state.
    pub fn is_visible(&self, visible: &BTreeSet<String>) -> bool {
        self.opaque || self.writes.iter().any(|w| visible.contains(w))
    }

    /// Whether two transitions can interfere, i.e. whether swapping their order is observable.
    ///
    /// Standard read-write conflict: one writes what the other reads or writes.
    pub fn conflicts_with(&self, other: &TransitionDeps) -> bool {
        if self.opaque || other.opaque {
            return true;
        }
        self.writes
            .iter()
            .any(|w| other.writes.contains(w) || other.reads.contains(w))
            || other
                .writes
                .iter()
                .any(|w| self.writes.contains(w) || self.reads.contains(w))
    }
}

/// An ample set, as indices into the enabled transition list.
///
/// The rule is the textbook one, deliberately kept to its simplest sound form:
///
/// - **C0** no transition enabled means the ample set is empty, so deadlock detection still
///   sees an empty successor list;
/// - **C1** if any enabled transition is visible, the ample set is everything;
/// - **C2** otherwise, take a single transition that conflicts with no other enabled transition.
///   Such a singleton is a persistent set: no transition outside it is dependent on it, which is
///   precisely the cycle proviso for safety.
///
/// Returning everything is always sound, so every doubt resolves the same way.
pub fn ample_set(deps: &[TransitionDeps], visible: &BTreeSet<String>) -> Vec<usize> {
    let n = deps.len();
    if n <= 1 {
        return (0..n).collect();
    }
    // C1: every transition in the ample set must be invisible to the claim. A singleton only
    // has to satisfy that for itself - an enabled VISIBLE transition does not force exploring
    // everything, as long as it is independent of the transition we pick. It stays enabled at
    // the successor and is taken from there.
    //
    // Bailing out whenever any visible transition was enabled used to cost most of the
    // reduction: on five independent processes with a claim over two of them, every state has a
    // visible transition enabled, so nothing was ever reduced.
    //
    // C2: a singleton that conflicts with nothing else is persistent.
    for i in 0..n {
        if (deps[i].internal || !deps[i].is_visible(visible))
            && (0..n).all(|j| j == i || !deps[i].conflicts_with(&deps[j]))
        {
            return vec![i];
        }
    }
    (0..n).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vis(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn empty_when_nothing_enabled() {
        assert!(ample_set(&[], &vis(&["x"])).is_empty());
    }

    #[test]
    fn single_enabled_transition_is_its_own_ample_set() {
        let d = [TransitionDeps::new(&[], &["_step_P"], true)];
        assert_eq!(ample_set(&d, &vis(&["x"])), vec![0]);
    }

    #[test]
    fn a_visible_but_independent_transition_does_not_force_the_whole_set() {
        // The second transition writes x, which the claim observes, but it touches nothing the
        // first one touches: taking the first still leaves it enabled.
        let d = [
            TransitionDeps::new(&[], &["_step_P"], true),
            TransitionDeps::new(&[], &["x"], false),
        ];
        assert_eq!(ample_set(&d, &vis(&["x"])), vec![0]);
    }

    #[test]
    fn a_visible_transition_that_conflicts_keeps_the_whole_set() {
        // Both write x: the visible one cannot be postponed past the other.
        let d = [
            TransitionDeps::new(&[], &["x"], false),
            TransitionDeps::new(&["x"], &["y"], false),
        ];
        assert_eq!(ample_set(&d, &vis(&["x"])), vec![0, 1]);
    }

    #[test]
    fn a_conflicting_pair_is_never_split() {
        // Both write x, so neither is independent of the other.
        let d = [
            TransitionDeps::new(&[], &["x"], false),
            TransitionDeps::new(&["x"], &["y"], false),
        ];
        assert_eq!(ample_set(&d, &vis(&["q"])), vec![0, 1]);
    }

    #[test]
    fn independent_internal_transitions_collapse_to_one() {
        let d = [
            TransitionDeps::new(&[], &["_step_P"], true),
            TransitionDeps::new(&[], &["_step_Q"], true),
        ];
        assert_eq!(ample_set(&d, &vis(&["x"])), vec![0]);
    }

    #[test]
    fn opaque_transitions_are_never_reduced() {
        let d = [
            TransitionDeps::new(&[], &["_step_P"], true),
            TransitionDeps::default(),
        ];
        assert_eq!(ample_set(&d, &vis(&["x"])), vec![0, 1]);
    }
}
