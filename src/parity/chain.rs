//! The consumer's chain shape, as fixtures.
//!
//! veriplan generates one active proctype per plan task, each carrying a failure branch, and one
//! ltl claim per requirement. On that shape the engine's state count doubles per task where pan's
//! is linear, because a bit the selected claim cannot read still distinguishes two states.
//!
//! ## The two probe directions are not interchangeable
//!
//! The same chain is generated twice, and the difference is only which way the claim points:
//!
//! - **refuting** `[] ( active_t1 -> done_tN )` - violated before `done_tN` can be set, so pan stops
//!   in 3 states and never pays for the failure bits.
//! - **closing** `[] ( _nr_pr >= 1 )` - holds in every reachable state, so the whole space must be
//!   explored. This is the direction that measures the state count.
//!
//! Our own count is identical in both directions (the search does not stop early), so a swapped
//! fixture is invisible in our output. That is why `tests` asserts the direction rather than
//! leaving it to a comment: a reviewer reading two green runs cannot tell them apart.

/// One chain fixture.
#[derive(Debug, Clone)]
pub struct Chain {
    /// Number of tasks in the chain.
    pub n: usize,
    /// Which direction the claim points. Recorded so a test can assert a fixture was not swapped.
    pub direction: Direction,
    /// The Promela source.
    pub source: String,
}

/// The two directions a probe can point. See the module docs for why this is not cosmetic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// `[] ( active_t1 -> done_tN )` - refuted early, so pan stops in 3 states.
    Refuting,
    /// `[] ( _nr_pr >= 1 )` - holds everywhere, so the space closes and the count is comparable.
    Closing,
}

impl Direction {
    /// The claim text for a chain of `n` tasks.
    pub fn claim(self, n: usize) -> String {
        match self {
            Self::Refuting => format!("[] ( active_t1_1 -> done_t1_{n} )"),
            Self::Closing => "[] ( _nr_pr >= 1 )".to_string(),
        }
    }

    /// A stable name fragment.
    pub fn label(self) -> &'static str {
        match self {
            Self::Refuting => "refuting",
            Self::Closing => "closing",
        }
    }
}

/// The task sizes the probes are measured at.
pub const SIZES: &[usize] = &[6, 8, 10, 12];

/// Build one chain fixture.
///
/// Each task `i` is guarded on its predecessor's completion flag, sets and clears its own active
/// bit, takes a branch that may set its failure bit, then sets its completion flag. The failure
/// bit is read by no claim in either direction, which is exactly the condition that makes it a
/// candidate for dropping from the state vector.
pub fn build(n: usize, direction: Direction) -> Chain {
    let mut source = String::new();
    for i in 1..=n {
        source.push_str(&format!("bit active_t1_{i} = 0;\n"));
        source.push_str(&format!("bit done_t1_{i} = 0;\n"));
        source.push_str(&format!("bit failed_t1_{i} = 0;\n"));
    }
    for i in 1..=n {
        let guard = if i == 1 {
            "(1)".to_string()
        } else {
            format!("(done_t1_{} == 1)", i - 1)
        };
        source.push_str(&format!(
            "active proctype task_1_{i}() {{\n\tdo\n\t:: {guard} ->\n\t\tactive_t1_{i} = 1;\n\t\tactive_t1_{i} = 0;\n\t\tif\n\t\t:: failed_t1_{i} = 1;\n\t\t:: skip;\n\t\tfi;\n\t\tdone_t1_{i} = 1;\n\t\tbreak\n\tod\n}}\n"
        ));
    }
    source.push_str(&format!("\nltl p0 {{ {} }}\n", direction.claim(n)));
    Chain {
        n,
        direction,
        source,
    }
}

/// Every fixture: each size in both directions.
pub fn all() -> Vec<Chain> {
    let mut out = Vec::new();
    for &n in SIZES {
        for d in [Direction::Refuting, Direction::Closing] {
            out.push(build(n, d));
        }
    }
    out
}

/// A measured row: our verdict and the oracle's, for one fixture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Measurement {
    pub n: usize,
    pub direction: Direction,
    /// Our error count, or None when the run gave no verdict.
    pub ours: Option<usize>,
    /// pan's error count, or None when the oracle was unavailable.
    pub pan_errors: Option<usize>,
    pub pan_states: Option<usize>,
}

/// Why a measurement fails the gate. Named rather than a bool, so a failure says which property.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateFailure {
    /// The verdicts disagree: the engines do not agree on this fixture at all.
    VerdictDisagreement,
    /// A verdict matched on a refuting fixture. Documented as NOT evidence of parity, because our
    /// count is identical in both directions and the match says only that we explored everything.
    MatchedOnRefutingProbe,
    /// The closing fixture did not close: it carries a violation, so it is measuring refutation
    /// depth instead of the state count.
    ClosingProbeRefuted,
    /// The two directions produced the same source, so one of them was swapped.
    DirectionsIdentical,
}

impl GateFailure {
    pub fn label(self) -> &'static str {
        match self {
            Self::VerdictDisagreement => "verdict disagreement",
            Self::MatchedOnRefutingProbe => "a matching count on a refuting probe is not parity",
            Self::ClosingProbeRefuted => {
                "the closing probe refuted, so it measures depth not count"
            }
            Self::DirectionsIdentical => {
                "the two probe directions are identical: a fixture was swapped"
            }
        }
    }
}

/// Check one measurement against the probe discipline. Returns every failure, not the first, so a
/// run reports what is wrong once rather than one defect per fix.
pub fn gate(rows: &[Measurement]) -> Vec<(usize, Direction, GateFailure)> {
    let mut out = Vec::new();
    for r in rows {
        // A closing probe that refutes is not a closing probe.
        if r.direction == Direction::Closing && r.pan_errors == Some(1) {
            out.push((r.n, r.direction, GateFailure::ClosingProbeRefuted));
        }
        // The refuting probe agrees on the verdict by construction. Recording the agreement as a
        // pass is the trap: it is true in both directions and distinguishes nothing.
        if r.direction == Direction::Refuting
            && r.ours.is_some()
            && r.pan_errors.is_some()
            && r.ours == r.pan_errors
        {
            out.push((r.n, r.direction, GateFailure::MatchedOnRefutingProbe));
        }
        if let (Some(o), Some(p)) = (r.ours, r.pan_errors)
            && o != p
            && r.direction == Direction::Closing
        {
            out.push((r.n, r.direction, GateFailure::VerdictDisagreement));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_fixture_carries_exactly_one_claim() {
        for c in all() {
            let n = c.source.matches("ltl ").count();
            assert_eq!(n, 1, "n={} {:?} carries {n} claims", c.n, c.direction);
        }
    }

    #[test]
    fn every_fixture_declares_one_triplet_per_task() {
        for c in all() {
            assert_eq!(c.source.matches("bit active_t1_").count(), c.n);
            assert_eq!(c.source.matches("bit done_t1_").count(), c.n);
            assert_eq!(c.source.matches("bit failed_t1_").count(), c.n);
            assert_eq!(c.source.matches("active proctype").count(), c.n);
        }
    }

    /// The property the projection is measured against depends on the failure bit being unread.
    /// If a claim ever names one, the fixture stops measuring projection and starts measuring the
    /// claim, silently.
    #[test]
    fn no_claim_reads_a_failure_bit() {
        for c in all() {
            let claim = c.source.split("ltl p0").nth(1).expect("claim absent");
            assert!(
                !claim.contains("failed_"),
                "n={} {:?} claim reads a failure bit: {claim}",
                c.n,
                c.direction
            );
        }
    }

    /// The direction is the whole point of having two fixtures. Assert it from the source rather
    /// than trusting the field, so a swap is caught even if the field is set consistently.
    #[test]
    fn the_two_directions_are_not_interchangeable() {
        for &n in SIZES {
            let r = build(n, Direction::Refuting).source;
            let c = build(n, Direction::Closing).source;
            assert_ne!(r, c, "n={n}: both directions produced the same source");
            assert!(
                r.contains(&format!("[] ( active_t1_1 -> done_t1_{n} )")),
                "n={n}: refuting fixture lost its claim"
            );
            assert!(
                c.contains("[] ( _nr_pr >= 1 )"),
                "n={n}: closing fixture lost its claim"
            );
            // The closing claim must not name the chain's own bits, or it would stop closing.
            let closing_claim = c.split("ltl p0").nth(1).expect("claim absent");
            assert!(
                !closing_claim.contains("done_t1_") && !closing_claim.contains("active_t1_"),
                "n={n}: closing claim names a chain bit, so it is no longer a closing probe"
            );
        }
    }

    /// A closing probe that can refute measures refutation depth, not the state count.
    #[test]
    fn the_closing_claim_holds_in_the_initial_state() {
        let c = build(6, Direction::Closing);
        assert!(c.source.contains("_nr_pr >= 1"));
        // _nr_pr is the number of running processes; six are active, so the guard is true.
        assert_eq!(c.source.matches("active proctype").count(), 6);
    }

    /// T1.6 / T1.7: the gate must not treat a matching count on a refuting probe as parity, and a
    /// swapped fixture must fail. These are hermetic - no pan needed - because the property under
    /// test is the discipline of the instrument, not the engine.
    #[test]
    fn a_match_on_the_refuting_probe_is_not_parity() {
        let rows = vec![Measurement {
            n: 6,
            direction: Direction::Refuting,
            ours: Some(1),
            pan_errors: Some(1),
            pan_states: Some(3),
        }];
        let failures = gate(&rows);
        assert_eq!(
            failures,
            vec![(6, Direction::Refuting, GateFailure::MatchedOnRefutingProbe)],
            "a verdict match on the refuting probe must not read as parity"
        );
    }

    #[test]
    fn a_closing_probe_that_refutes_fails_the_gate() {
        let rows = vec![Measurement {
            n: 6,
            direction: Direction::Closing,
            ours: Some(1),
            pan_errors: Some(1),
            pan_states: Some(3),
        }];
        let failures = gate(&rows);
        assert!(
            failures
                .iter()
                .any(|(_, _, f)| *f == GateFailure::ClosingProbeRefuted),
            "a closing probe that refutes measures the wrong thing: {failures:?}"
        );
    }

    #[test]
    fn a_correct_run_passes_the_gate() {
        let rows = vec![
            Measurement {
                n: 6,
                direction: Direction::Refuting,
                ours: Some(1),
                pan_errors: Some(1),
                pan_states: Some(3),
            },
            Measurement {
                n: 6,
                direction: Direction::Closing,
                ours: Some(0),
                pan_errors: Some(0),
                pan_states: Some(37),
            },
        ];
        let failures = gate(&rows);
        // Only the refuting match is flagged; the closing row is clean.
        assert_eq!(failures.len(), 1, "{failures:?}");
        assert_eq!(failures[0].1, Direction::Refuting);
    }

    /// The swap this whole module exists to make impossible: if the two directions ever produce
    /// the same source, the gate must say so rather than measuring whichever one survived.
    #[test]
    fn swapped_directions_are_detected() {
        for &n in SIZES {
            let same = build(n, Direction::Refuting).source == build(n, Direction::Closing).source;
            assert!(
                !same,
                "n={n}: the two directions are identical, a fixture was swapped"
            );
        }
    }
}
