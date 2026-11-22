//! Differential tests for partial order reduction.
//!
//! A reduction bug does not fail loudly: it produces a green verdict on a broken model. So every
//! test here compares the reduced search against the unreduced one we already trust, on verdicts
//! rather than on state counts. State counts are reported, but only asserted where the point of
//! the test is that reduction happened at all.

use crate::engine::checker::{CheckResult, CheckerConfig, UnverifiedKind};

fn run(src: &str, por: bool, max_states: usize) -> CheckResult {
    let cfg = CheckerConfig {
        por_enabled: por,
        max_states,
        ..Default::default()
    };
    crate::verify_with_config(src, &cfg).expect("fixture must load and explore")
}

/// Description class: the kind of finding, not its wording or the state it names.
fn class(desc: &str) -> String {
    let head = desc.split(':').next().unwrap_or(desc).trim();
    head.chars().take(40).collect()
}

fn verdicts(r: &CheckResult) -> (Vec<String>, Vec<String>) {
    let mut v: Vec<String> = r
        .violations
        .iter()
        .map(|x| format!("{}|{}", x.property_name, class(&x.description)))
        .collect();
    v.sort();
    v.dedup();
    let mut u: Vec<String> = r
        .unverified
        .iter()
        .map(|x| format!("{}|{}", x.property_name, x.kind.as_str()))
        .collect();
    u.sort();
    u.dedup();
    (v, u)
}

/// Five processes, three steps each, nothing to observe: the shape where reduction has to work.
const INDEPENDENT: &str = "
byte a1, a2, a3, a4, a5;
active proctype P1() { do :: (1) -> a1 = 1; a1 = 2; a1 = 3; break od }
active proctype P2() { do :: (1) -> a2 = 1; a2 = 2; a2 = 3; break od }
active proctype P3() { do :: (1) -> a3 = 1; a3 = 2; a3 = 3; break od }
active proctype P4() { do :: (1) -> a4 = 1; a4 = 2; a4 = 3; break od }
active proctype P5() { do :: (1) -> a5 = 1; a5 = 2; a5 = 3; break od }
 ";

/// Each task runs only after its predecessor completed: a chain of dependent guards.
const CHAIN: &str = "
bool t1, t2, t3, t4;
active proctype T1() { do :: (1) -> t1 = 1; break od }
active proctype T2() { do :: (t1) -> t2 = 1; break od }
active proctype T3() { do :: (t2) -> t3 = 1; break od }
active proctype T4() { do :: (t3) -> t4 = 1; break od }
ltl p1 { [] ( t2 -> t1 ) }
ltl p2 { [] ( t4 -> t3 ) }
 ";

/// Two phases separated by a barrier.
const CONCURRENT_PHASE: &str = "
bool ready_a, ready_b, go, finished;
active proctype A() { do :: (1) -> ready_a = 1; (ready_b); go = 1; (finished); break od }
active proctype B() { do :: (1) -> ready_b = 1; (go); finished = 1; break od }
ltl p1 { [] ( finished -> ready_a ) }
ltl p2 { [] ( go -> ready_b ) }
 ";

/// The consumer's own chain shape: one proctype per task, guarded on the predecessor's
/// completion flag, body activate/deactivate/complete, with one and with two claims.
const VERIPLAN_ONE: &str = "
bool t1_done, t2_done;
active proctype task_t1() { do :: (1) -> t1_done = 1; break od }
active proctype task_t2() { do :: (t1_done) -> t2_done = 1; break od }
ltl p0 { [] ( t2_done -> t1_done ) }
 ";

const VERIPLAN_TWO: &str = "
bool t1_done, t2_done, t3_done;
active proctype task_t1() { do :: (1) -> t1_done = 1; break od }
active proctype task_t2() { do :: (t1_done) -> t2_done = 1; break od }
active proctype task_t3() { do :: (t2_done) -> t3_done = 1; break od }
ltl p0 { [] ( t2_done -> t1_done ) }
ltl p1 { [] ( t3_done -> t2_done ) }
 ";

/// The violation sits behind a guard that reads another process's write. An ample set that
/// ignores read-write conflicts would skip the guard and report the model clean.
const DEPENDENT_GUARD: &str = "
bool done1, bad;
active proctype T1() { do :: (1) -> done1 = 1; break od }
active proctype T2() { do :: (done1) -> bad = 1; break od }
ltl p0 { [] ( !bad ) }
 ";

/// Five independent processes with a claim over two of them. This is the case where the ample
/// set has to reduce even though a visible transition is enabled somewhere in the state - the
/// shape that exposed an over-strict C1 (2510 states before the relaxation, 20 after).
const INDEPENDENT_WITH_CLAIM: &str = "
bool b1, b2, b3, b4, b5;
active proctype P1() { do :: (1) -> b2 = 1; b1 = 1; break od }
active proctype P2() { do :: (1) -> b3 = 1; b3 = 0; b3 = 1; break od }
active proctype P3() { do :: (1) -> b4 = 1; b4 = 0; b4 = 1; break od }
active proctype P4() { do :: (1) -> b5 = 1; b5 = 0; b5 = 1; break od }
active proctype P5() { do :: (1) -> b5 = 0; b5 = 1; b5 = 0; break od }
ltl p0 { [] ( b1 -> b2 ) }
 ";

/// A process looping invisibly next to a violation that is two invisible steps away.
///
/// This is the shape that broke the first version of the reduction: the ample set was persistent
/// at every state, so each state kept only its own process's transition, the DFS never returned
/// to the state where the other process was dropped, and the violation was never reached. It
/// reported two states and zero errors against the unreduced search's one. The cycle proviso
/// exists to stop exactly this, and this fixture is why it is not optional.
const CYCLE_HIDDEN_VIOLATION: &str = "
byte tmp; bool bad;
active proctype A() { do :: (1) -> skip od }
active proctype B() { do :: (1) -> tmp = 1; bad = 1; break od }
ltl p { [] ( !bad ) }
 ";

/// A violated liveness claim whose only cycle consists of internal transitions.
const LIVENESS: &str = "
bool flag;
active proctype P() { do :: (1) -> skip od }
ltl l0 { <> ( flag ) }
 ";

/// Fixture, source, and how many traversals reduction runs over it. The last number matters
/// because states_explored is a sum over per-claim traversals: a two-claim model may legitimately
/// report twice the states of one unreduced pass.
const CORPUS: &[(&str, &str, usize)] = &[
    ("independent", INDEPENDENT, 1),
    ("independent_with_claim", INDEPENDENT_WITH_CLAIM, 1),
    ("chain", CHAIN, 2),
    ("concurrent_phase", CONCURRENT_PHASE, 2),
    ("veriplan_one", VERIPLAN_ONE, 1),
    ("veriplan_two", VERIPLAN_TWO, 2),
    ("dependent_guard", DEPENDENT_GUARD, 1),
    ("cycle_hidden_violation", CYCLE_HIDDEN_VIOLATION, 1),
    ("liveness", LIVENESS, 1),
];

#[test]
fn reduction_preserves_verdicts_on_the_whole_corpus() {
    for (name, src, traversals) in CORPUS {
        let full = run(src, false, 1_000_000);
        let reduced = run(src, true, 1_000_000);
        assert_eq!(
            verdicts(&full),
            verdicts(&reduced),
            "reduction changed the verdict for {name}: full={:?} reduced={:?}",
            verdicts(&full),
            verdicts(&reduced)
        );
        // Each claim gets its own traversal, so the sum is bounded by the unreduced count times
        // the number of traversals - not by one unreduced pass.
        assert!(
            reduced.states_explored <= full.states_explored * traversals,
            "reduction explored more than its per-claim bound on {name}: {} vs {} x {}",
            reduced.states_explored,
            full.states_explored,
            traversals
        );
    }
}

#[test]
fn reduction_actually_shrinks_independent_processes() {
    let full = run(INDEPENDENT, false, 1_000_000);
    let reduced = run(INDEPENDENT, true, 1_000_000);
    // The exact unreduced count depends on how many steps a body is laid out in, which the
    // flattening changed (3 statements used to be 3 transitions, now they are steps plus a
    // halt). What matters here is that reduction still collapses the space.
    assert!(
        full.states_explored > reduced.states_explored,
        "the reduced search must be smaller"
    );
    // Reduction still collapses this shape, but by a smaller factor than it used to: 16807 ->
    // 4807, where the step machine gave 3125. Bodies are laid out as one step per statement, so
    // there are more states at which the cycle proviso has to expand the whole enabled set, and
    // that proviso is what keeps the reduction sound. The factor is recorded rather than asserted
    // at the old value: a body that carries more steps per process is not a regression in the
    // reduction, and pinning the ratio to the old number would make any change to the encoding
    // look like one. What must not change is that reduction happens at all.
    assert!(
        reduced.states_explored * 2 < full.states_explored,
        "reduction should still collapse independent processes, got {} vs {}",
        reduced.states_explored,
        full.states_explored
    );
}

#[test]
fn a_dependent_guard_is_not_reduced_away() {
    let full = run(DEPENDENT_GUARD, false, 1_000_000);
    let reduced = run(DEPENDENT_GUARD, true, 1_000_000);
    assert!(
        !full.violations.is_empty(),
        "fixture must violate its claim"
    );
    assert_eq!(verdicts(&full), verdicts(&reduced));
    assert!(
        reduced.violations.iter().any(|v| v.property_name == "p0"),
        "the violation behind the dependent guard must survive: {:?}",
        reduced.violations
    );
}

#[test]
fn an_invisible_loop_does_not_hide_a_violation_behind_it() {
    let full = run(CYCLE_HIDDEN_VIOLATION, false, 1_000_000);
    let reduced = run(CYCLE_HIDDEN_VIOLATION, true, 1_000_000);
    assert!(
        !full.violations.is_empty(),
        "the fixture must violate its claim without reduction"
    );
    assert_eq!(
        verdicts(&full),
        verdicts(&reduced),
        "a process looping invisibly must not hide the other process's violation"
    );
    assert!(reduced.violations.iter().any(|v| v.property_name == "p"));
}

#[test]
fn an_internal_accepting_cycle_still_refutes_a_liveness_claim() {
    let full = run(LIVENESS, false, 1_000_000);
    let reduced = run(LIVENESS, true, 1_000_000);
    assert!(!full.violations.is_empty(), "fixture must violate l0");
    assert_eq!(verdicts(&full), verdicts(&reduced));
    assert!(reduced.violations.iter().any(|v| v.property_name == "l0"));
}

#[test]
fn the_state_budget_bounds_the_whole_run_and_names_starved_claims() {
    // Three claims, a budget too small to finish one full traversal, let alone three.
    let src = "
bool a, b, c, d;
active proctype P1() { do :: (1) -> a = 1; a = 2; break od }
active proctype P2() { do :: (1) -> b = 1; b = 2; break od }
active proctype P3() { do :: (1) -> c = 1; c = 2; break od }
active proctype P4() { do :: (1) -> d = 1; d = 2; break od }
ltl p0 { [] ( !a ) }
ltl p1 { [] ( !b ) }
ltl p2 { [] ( !c ) }
 ";
    let r = run(src, true, 2);
    let claims = ["p0", "p1", "p2"];
    // The invariant: no claim may be absent. Each is either refuted or reported undecided.
    //
    // These claims are single atoms and every one of them is genuinely refutable, so a claim that
    // is neither refuted nor starved would be one the run dropped. They used to be comparison
    // atoms, which were always reported undecided - so the invariant held for the wrong reason:
    // it passed because nothing could be decided, not because nothing was lost.
    for c in claims {
        let reported = r.violations.iter().any(|v| v.property_name == c)
            || r.unverified.iter().any(|u| u.property_name == c);
        assert!(reported, "claim {c} was silently absent from the result");
    }
    assert!(
        r.unverified
            .iter()
            .any(|u| u.kind == UnverifiedKind::StateBudget),
        "an exhausted budget must be reported as StateBudget: {:?}",
        r.unverified
    );
    assert!(
        r.states_explored <= 2,
        "the budget must bound the whole run, got {}",
        r.states_explored
    );
}

#[test]
fn every_claim_survives_an_early_stop() {
    let cfg = CheckerConfig {
        por_enabled: true,
        stop_at_first_violation: true,
        ..Default::default()
    };
    let src = "
bool t1, t2, bad;
active proctype T1() { do :: (1) -> t1 = 1; bad = 1; break od }
active proctype T2() { do :: (t1) -> t2 = 1; break od }
ltl p0 { [] ( !bad ) }
ltl p1 { [] ( t2 -> t1 ) }
 ";
    let r = crate::verify_with_config(src, &cfg).expect("fixture must load");
    for c in ["p0", "p1"] {
        let reported = r.violations.iter().any(|v| v.property_name == c)
            || r.unverified.iter().any(|u| u.property_name == c);
        assert!(
            reported,
            "claim {c} was silently absent after an early stop"
        );
    }
}
/// Locate a program on PATH.
fn have(prog: &str) -> bool {
    std::process::Command::new(prog)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
}

/// Number of claims a fixture declares, in declaration order.
fn claim_names(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in src.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("ltl ") {
            if let Some(name) = rest.split_whitespace().next() {
                out.push(name.to_string());
            }
        }
    }
    out
}

fn pan_errors(pan: &std::path::Path, dir: &std::path::Path, claim: Option<&str>) -> usize {
    let mut cmd = std::process::Command::new(pan);
    match claim {
        // -a is not optional: without it pan reports "acceptance cycles - (not selected)" and
        // returns zero errors for every liveness claim, which would make the oracle agree with
        // a checker that never looks for cycles.
        Some(name) => {
            cmd.arg("-a").arg("-N").arg(name);
        }
        None => {
            cmd.arg("-n");
        }
    };
    let out = cmd.current_dir(dir).output().expect("pan must run");
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        if let Some(pos) = line.find("errors: ") {
            let rest = &line[pos + 8..];
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(n) = digits.parse() {
                return n;
            }
        }
    }
    panic!("pan printed no error count:\n{}", text);
}

/// The oracle: for every fixture, our error count must match pan's, per claim.
///
/// pan checks one claim per traversal, so the comparison sums its per-claim counts. That is the
/// same quantity the parity metric measures, and it is the only form in which our aggregated
/// result and pan's per-claim runs are comparable.
#[test]
fn pan_agrees_with_our_error_counts_on_the_whole_corpus() {
    if !have("spin") || !have("gcc") {
        eprintln!("spin or gcc not found on PATH; skipping the oracle comparison");
        return;
    }
    for (name, src, _) in CORPUS {
        let dir = std::env::temp_dir().join(format!("spin_rs_por_{name}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("m.pml"), src).unwrap();

        let status = std::process::Command::new("spin")
            .arg("-a")
            .arg("m.pml")
            .current_dir(&dir)
            .stdout(std::process::Stdio::null())
            .status()
            .expect("spin must run");
        assert!(status.success(), "spin -a failed for {name}");
        let status = std::process::Command::new("gcc")
            .args(["-w", "-o", "pan", "pan.c"])
            .current_dir(&dir)
            .status()
            .expect("gcc must run");
        assert!(status.success(), "gcc failed for {name}");

        let pan = dir.join("pan");
        let claims = claim_names(src);
        let oracle: usize = if claims.is_empty() {
            pan_errors(&pan, &dir, None)
        } else {
            claims.iter().map(|c| pan_errors(&pan, &dir, Some(c))).sum()
        };

        let full = run(src, false, 1_000_000);
        let reduced = run(src, true, 1_000_000);
        assert_eq!(
            full.errors, oracle,
            "{name}: unreduced search disagrees with pan"
        );
        assert_eq!(
            reduced.errors, oracle,
            "{name}: reduced search disagrees with pan"
        );
    }
}

/// A deterministic sweep over a small family of shapes, rather than one fixture per shape.
///
/// The cycle proviso bug that shipped in v0.4.0 was found by a caller's model, not by this
/// corpus, and the reason is structural: every fixture here was a terminated process, and a
/// terminated process has no cycle to hide a transition behind. A curated fixture list can only
/// contain the shapes someone thought of. This sweep enumerates the small family that did contain
/// the bug - looping and terminating processes crossed with the claims they violate - so the next
/// proviso mistake fails a test instead of reaching a consumer.
#[test]
fn reduction_preserves_verdicts_across_a_sweep_of_small_shapes() {
    let claims = [
        "[] ( !(fA && fB) )",
        "[] ( fA -> fB )",
        "[] ( !fB )",
        "[] ( fA )",
        "[] ( !fA )",
    ];
    let mut checked = 0usize;
    for a_loops in [false, true] {
        for b_loops in [false, true] {
            for b_guarded in [false, true] {
                for claim in claims {
                    let a_body = if a_loops {
                        "do :: (1) -> fA = 1 od"
                    } else {
                        "do :: (1) -> fA = 1; break od"
                    };
                    let b_guard = if b_guarded { "fA" } else { "1" };
                    let b_body = if b_loops {
                        format!("do :: ({b_guard}) -> fB = 1 od")
                    } else {
                        format!("do :: ({b_guard}) -> fB = 1; break od")
                    };
                    let src = format!(
                        "bool fA, fB;\nactive proctype A() {{ {a_body} }}\nactive proctype B() {{ {b_body} }}\nltl p {{ {claim} }}\n"
                    );
                    let full = run(&src, false, 1_000_000);
                    let reduced = run(&src, true, 1_000_000);
                    assert_eq!(
                        verdicts(&full),
                        verdicts(&reduced),
                        "reduction changed the verdict for a_loops={a_loops} b_loops={b_loops} b_guarded={b_guarded} claim={claim}\n{src}"
                    );
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 40, "the sweep must actually enumerate its shapes");
}
