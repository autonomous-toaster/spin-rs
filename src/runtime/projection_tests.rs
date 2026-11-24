//! Tests for state projection, the liveness budget, reduction, and batched expansion.
//!
//! Split from `tests.rs` so neither module grows past what a reader can hold. The subject here is
//! the set of fields that decide state identity and reduction visibility - the two questions that
//! were once answered by one field, unsoundly in both directions - plus the throughput and guard
//! semantics of the batched expansion.

// ─── Claim-relative state projection ───────────────────────────────────────

/// Two claims with different observable sets must produce different state counts on one model.
///
/// This is what makes the projection claim-relative rather than a global dead-variable pass. A
/// global pass would retain the union of both claims' fields, so both counts would be equal to the
/// unreduced count and the property below would fail.
#[test]
fn projection_is_claim_relative() {
    // `only_a` reads a; `only_b` reads b. Neither names the other's bit.
    let source = "\
bit a = 0;
bit b = 0;
active proctype P() {
    do :: (1) ->
        if
        :: a = 1; a = 0;
        :: b = 1; b = 0;
        fi;
        break
    od
}
ltl reads_a { [] ( a == 0 ) }
ltl reads_b { [] ( b == 0 ) }
";
    let model = crate::LuaModel::from_source(source).unwrap();
    use crate::engine::checker::Model as _;
    let scopes = model.reduction_scopes();
    let a_scope = scopes
        .iter()
        .find(|s| s.name == "reads_a")
        .expect("reads_a scope");
    let b_scope = scopes
        .iter()
        .find(|s| s.name == "reads_b")
        .expect("reads_b scope");
    assert!(a_scope.visible.contains("a"), "reads_a must observe a");
    assert!(!a_scope.visible.contains("b"), "reads_a must not observe b");
    assert!(b_scope.visible.contains("b"), "reads_b must observe b");
    assert!(!b_scope.visible.contains("a"), "reads_b must not observe a");
    assert_ne!(
        a_scope.visible, b_scope.visible,
        "the two claims must not share one global visible set"
    );
    // The identity sets must differ for the same reason: it is the per-claim projection that makes
    // the counts differ, and a global identity set would buy nothing.
    assert_ne!(
        a_scope.identity, b_scope.identity,
        "identity must be claim-relative too, not the union of both claims"
    );
}

/// A hidden bit cannot suppress an assertion failure.
///
/// The assertion marker is in every claim's visible set, so projecting on a claim that does not
/// name it still cannot merge a state where the assertion failed with one where it did not.
#[test]
fn a_hidden_bit_cannot_hide_an_assertion_failure() {
    let source = "\
bit other = 0;
active proctype P() {
    other = 1;
    assert(false)
}
ltl p0 { [] ( other == 0 ) }
";
    let result = crate::verify(source).expect("the model must run");
    let found = result
        .violations
        .iter()
        .any(|v| v.description.contains("assertion"));
    assert!(
        found,
        "an assertion failure must be reported even under a claim that does not name it: {:?}",
        result
            .violations
            .iter()
            .map(|v| v.description.clone())
            .collect::<Vec<_>>()
    );
}

/// A hidden bit cannot suppress a deadlock.
///
/// With no claim, the projection is the identity over the whole state, so a deadlock detectable
/// only from a variable no claim names is still found.
#[test]
fn a_hidden_bit_cannot_hide_a_deadlock() {
    // Two processes: one blocks forever on a guard that never fires.
    let source = "\
bit flagx = 0;
active proctype A() { do :: (flagx == 1) -> skip od }
active proctype B() { do :: (flagx == 1) -> skip od }
";
    let result = crate::verify(source).expect("the model must run");
    let found = result
        .violations
        .iter()
        .any(|v| v.description.contains("deadlock"));
    assert!(
        found,
        "a deadlock must be reported: {:?}",
        result
            .violations
            .iter()
            .map(|v| v.description.clone())
            .collect::<Vec<_>>()
    );
}

/// A field a guard branches on cannot be projected away, even when no claim names it.
///
/// Measured failure this asserts against: projecting onto the claim's variables dropped `fA` and
/// lost the violation on a two-process model where B's guard reads fA.
#[test]
fn a_guard_field_is_never_projected_away() {
    let source = "\
bool fA, fB;
active proctype A() { do :: (1) -> fA = 1 od }
active proctype B() { do :: (fA) -> fB = 1; break od }
ltl p { [] ( !fB ) }
";
    let model = crate::LuaModel::from_source(source).unwrap();
    use crate::engine::checker::Model as _;
    let scope = model
        .reduction_scopes()
        .into_iter()
        .find(|s| s.name == "p")
        .expect("a scope for p");
    // Identity, not reduction visibility. Reduction keeps the claim's own variables because it
    // may commute transitions that only touch others; identity must keep fA because merging
    // states that differ in it loses B's transition entirely.
    assert!(
        scope.identity.contains("fA"),
        "fA is read by B's guard, so it distinguishes states: {:?}",
        scope.identity
    );
    // And the reduction set stays narrow, or POR stops reducing. Asserting both is what keeps the
    // two sets from being merged back together by a later edit.
    assert!(
        !scope.visible.contains("fA"),
        "the claim does not name fA, so reduction must still see it as invisible: {:?}",
        scope.visible
    );
}

// ─── Liveness budget ───────────────────────────────────────────────────────

/// A ten-task chain with one <> claim, as the consumer generates it.
fn liveness_chain(n: usize) -> String {
    let mut s = String::new();
    for i in 1..=n {
        s.push_str(&format!("bit active_t{i} = 0;\nbit done_t{i} = 0;\n"));
    }
    for i in 1..=n {
        let guard = if i == 1 {
            "(1)".to_string()
        } else {
            format!("(done_t{} == 1)", i - 1)
        };
        s.push_str(&format!(
            "active proctype T{i}() {{\n\tdo\n\t:: {guard} ->\n\t\tactive_t{i} = 1;\n\t\tactive_t{i} = 0;\n\t\tdone_t{i} = 1;\n\t\tbreak\n\tod\n}}\n"
        ));
    }
    s.push_str(&format!(
        "\nltl p0 {{ [] ( failed_t1 -> <> active_t{n} ) }}\n"
    ));
    s.push_str("bit failed_t1 = 0;\n");
    s
}

/// T3.5: a one-state budget on a liveness claim must terminate and name the claim.
///
/// Before the ceiling existed this ran past 45 s, so the test is as much a termination assertion
/// as a reporting one. It is deliberately not a wall-clock assertion: a slow machine must not
/// fail a correctness gate, and termination is what the caller's fuse actually needs.
#[test]
fn a_liveness_claim_is_bounded_and_named() {
    let source = liveness_chain(10);
    let config = crate::engine::checker::CheckerConfig {
        max_states: 1,
        ..Default::default()
    };
    let result = crate::verify_with_config(&source, &config)
        .expect("the model must terminate: an unbounded liveness search is the defect");
    let named = result.unverified.iter().any(|u| {
        u.property_name == "p0" && u.kind == crate::engine::checker::UnverifiedKind::StateBudget
    });
    assert!(
        named,
        "the liveness claim must be reported as StateBudget by name: {:?}",
        result
            .unverified
            .iter()
            .map(|u| (u.property_name.clone(), u.kind.as_str()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        result.errors, 0,
        "a budget-exhausted claim is not a violation either"
    );
}

/// T3.6: a liveness claim that closes within budget keeps its verdict.
#[test]
fn a_liveness_claim_that_closes_keeps_its_verdict() {
    // x becomes 1 in every run, so <> ( x == 1 ) holds.
    let source = "\
bool x = 0;
active proctype P() { x = 1; do :: (1) -> skip od }
ltl p0 { <> ( x == 1 ) }
";
    let result = crate::verify(source).expect("the model must run");
    assert!(
        result.unverified.is_empty(),
        "a claim that closes must not be reported undecided: {:?}",
        result.unverified
    );
    assert_eq!(
        result.errors, 0,
        "the claim holds, so there is no violation"
    );
}

/// T3.6, the other direction: a violated liveness claim is still refuted.
#[test]
fn a_violated_liveness_claim_is_still_refuted() {
    let source = "\
bool x = 0;
active proctype P() { do :: (1) -> skip od }
ltl p0 { <> ( x == 1 ) }
";
    let result = crate::verify(source).expect("the model must run");
    assert!(
        !result.unverified.iter().any(|u| u.property_name == "p0"),
        "a refuted claim must not also be reported undecided: {:?}",
        result.unverified
    );
    assert!(result.errors >= 1, "the claim is violated");
}

/// T3.10: a budget of zero must not silently become a positive budget.
///
/// The run refuses rather than returning a verdict. That is the strongest honest answer: the
/// existing "a run that explored nothing is not a verdict" guard in `lib.rs` already covers it,
/// and a refusal is the one classification no caller can count as satisfied.
#[test]
fn a_zero_budget_is_refused_not_raised() {
    let source = "\
bit a = 0;
active proctype P() { do :: (1) -> a = 1 od }
ltl p1 { [] ( a == 0 ) }
";
    let config = crate::engine::checker::CheckerConfig {
        max_states: 0,
        ..Default::default()
    };
    let err = crate::verify_with_config(source, &config)
        .expect_err("a budget of zero must not return a verdict");
    let text = err.to_string();
    assert!(
        text.contains("no state was explored"),
        "the refusal must say nothing was checked, got: {text}"
    );
    // The old behaviour raised the budget to one state per claim and explored anyway.
    assert!(
        !text.contains("3 states"),
        "a zero budget must not be raised silently: {text}"
    );
}

/// T3.8: a budget below the claim count is raised, and the run says so.
#[test]
fn a_budget_below_the_claim_count_is_reported() {
    let source = "\
bit a = 0;
active proctype P() { do :: (1) -> a = 1; a = 0 od }
ltl p1 { [] ( a == 0 ) }
ltl p2 { [] ( a >= 0 ) }
ltl p3 { [] ( a <= 1 ) }
";
    let config = crate::engine::checker::CheckerConfig {
        max_states: 2,
        ..Default::default()
    };
    let result = crate::verify_with_config(source, &config).expect("the model must run");
    // Every claim is named, none is silently dropped.
    let names: Vec<&str> = result
        .unverified
        .iter()
        .map(|u| u.property_name.as_str())
        .collect();
    for want in ["p1", "p2", "p3"] {
        assert!(
            names.contains(&want),
            "{want} must be reported undecided, got {names:?}"
        );
    }
    // The floor is disclosed rather than left for the caller to infer.
    assert!(
        result
            .unverified
            .iter()
            .any(|u| u.reason.contains("one state per claim")),
        "the raised budget must be stated: {:?}",
        result.unverified.first().map(|u| u.reason.clone())
    );
}

/// T4.5, as a measurement rather than a gate: report states/s on the committed chain fixture.
///
/// This is NOT asserted at the 1e5 floor. The floor is a real requirement and it is not met: the
/// work that reaches it is a structured state (task 4.3), not the hoisting done in 4.1. Asserting
/// the number here would either fail the suite or be tuned until it passed, and a throughput gate
/// that passes by being made easier is the thing this change exists to prevent.
#[test]
fn chain_throughput_is_measured() {
    use crate::parity::chain::{Direction, build};
    // A chain large enough that the run is not dominated by model setup. At N=8 the fixture
    // explores 65 states, and 65 states measure startup rather than throughput - a rate computed
    // from them says more about Lua initialisation than about the search.
    let source = build(12, Direction::Closing).source;
    let config = crate::engine::checker::CheckerConfig::default();
    // Warm, then measure once: a single cold run is dominated by Lua setup.
    let _ = crate::verify_with_config(&source, &config);
    let start = std::time::Instant::now();
    let result = crate::verify_with_config(&source, &config).expect("the fixture must run");
    let secs = start.elapsed().as_secs_f64();
    let rate = result.states_explored as f64 / secs.max(1e-9);
    eprintln!(
        "chain N=12 closing: {} states in {:.4}s = {:.0} states/s (pan: 1e5-1e6; floor 1e5)",
        result.states_explored, secs, rate
    );
    // The state count is the property this change fixes, and it is asserted.
    assert!(
        result.states_explored < 200,
        "the projection must keep the chain linear: {} states",
        result.states_explored
    );
}

// ─── Reduction must not silently switch itself off ─────────────────────────
//
// A verdict gate cannot see a reduction regression: the answers stay right and only the cost
// moves. Measured once already - the state projection put control-flow fields into the set the
// ample-set computation reads, every transition became "visible", and POR stopped reducing on
// every fan-out model while 166 parity models, 327 unit tests and all the chain fixtures stayed
// green. These tests assert the count, not only the verdict.

/// N independent proctypes, one branch each, and a claim that reads none of the branches.
fn fan_out(n: usize, claim: &str) -> String {
    let mut s = String::new();
    for i in 1..=n {
        s.push_str(&format!(
            "bit active_{i} = 0;\nbit done_{i} = 0;\nbit failed_{i} = 0;\n"
        ));
    }
    for i in 1..=n {
        s.push_str(&format!(
            "active proctype T{i}() {{\n\tactive_{i} = 1;\n\tactive_{i} = 0;\n\tif\n\t:: failed_{i} = 1;\n\t:: skip;\n\tfi;\n\tdone_{i} = 1\n}}\n"
        ));
    }
    s.push_str(&format!("\nltl p0 {{ {claim} }}\n"));
    s
}

/// Reduction must actually reduce, and by a wide margin.
///
/// The bound is stated as "at least half" rather than an exact count: the exact number depends on
/// how many transitions are commutable at each state, which is a property of the fixture, and a
/// hard-coded count would fail on an unrelated generator change for no reason. Half is far above
/// the 3% that "reduction silently off" produced (704 of 729) and far below what correct reduction
/// achieves, so it separates the two conditions without pinning the fixture.
#[test]
fn reduction_actually_reduces_a_fan_out() {
    for n in [3usize, 4] {
        let source = fan_out(n, "[] ( _nr_pr >= 1 )");
        let reduced =
            crate::verify_with_config(&source, &crate::engine::checker::CheckerConfig::default())
                .expect("reduced run")
                .states_explored;
        let unreduced_run = crate::verify_with_config(
            &source,
            &crate::engine::checker::CheckerConfig {
                por_enabled: false,
                ..Default::default()
            },
        )
        .expect("unreduced run");
        // A capped run is not a measurement. If both sides stop at the ceiling the ratio is one
        // and the comparison silently inverts: capping a slow test to make it usable makes it
        // vacuous, and this assertion is what stops that. Same defect class as a ceiling chosen
        // far from the observed value - both leave a test that cannot fail.
        assert!(
            !unreduced_run.unverified.iter().any(|u| matches!(
                u.kind,
                crate::engine::checker::UnverifiedKind::StateBudget
                    | crate::engine::checker::UnverifiedKind::DepthBudget
            )),
            "n={n}: the unreduced run hit a budget, so its count is a ceiling and not a \
             measurement; the ratio against it means nothing"
        );
        let unreduced = unreduced_run.states_explored;
        assert!(unreduced > 0, "n={n}: the unreduced run explored nothing");
        assert!(
            reduced * 10 < unreduced,
            "n={n}: reduction explored {reduced} of {unreduced} states - partial order reduction \
             has effectively stopped reducing. Check that ReductionScope::visible still holds \
             only the claim's own variables: adding the control-flow or guard fields there makes \
             every transition visible and no ample set can be formed."
        );
    }
}

/// Reduction must not change a verdict, on shapes where the claim does read the flags.
///
/// This is the other half: the test above proves reduction happens, this one proves it is sound.
/// A reduction that reduces too much passes the first and fails here.
#[test]
fn reduction_agrees_with_the_unreduced_search_on_a_fan_out() {
    let claims = [
        "[] ( _nr_pr >= 1 )",
        "[] ( done_1 == 0 || active_1 == 1 )",
        "[] ( failed_1 == 0 )",
        "[] ( done_2 == 1 || done_1 == 0 )",
    ];
    for claim in claims {
        for n in [2usize, 3] {
            let source = fan_out(n, claim);
            let reduced = crate::verify_with_config(
                &source,
                &crate::engine::checker::CheckerConfig::default(),
            )
            .expect("reduced run");
            let unreduced = crate::verify_with_config(
                &source,
                &crate::engine::checker::CheckerConfig {
                    por_enabled: false,
                    ..Default::default()
                },
            )
            .expect("unreduced run");
            assert_eq!(
                reduced.errors, unreduced.errors,
                "n={n} claim={claim}: reduction changed the verdict ({} vs {})",
                reduced.errors, unreduced.errors
            );
        }
    }
}

// ─── Liveness claims are projected for state identity ──────────────────────
//
// A liveness claim is never REDUCED (an accepting cycle can be invisible), but it is still
// projected: merging states the claim cannot tell apart does not remove a cycle, it only stops
// the search counting states that differ solely in fields no guard and no claim reads.

/// A liveness claim that cannot read the failure bits must not pay for them.
///
/// Before the projection, the same model at eight tasks explored 240296 states; with it, 2041.
///
/// The bound sits between the two measured values with margin on both sides: 2041 passes, 240296
/// fails, and 10000 is an order of magnitude below the broken figure rather than a rounding of it.
/// Measured both, then chose - a bound set from the good value alone is a guess.
#[test]
fn a_liveness_claim_does_not_pay_for_bits_it_cannot_read() {
    let mut s = String::new();
    for i in 1..=8 {
        s.push_str(&format!(
            "bit active_t{i} = 0;\nbit done_t{i} = 0;\nbit failed_t{i} = 0;\n"
        ));
    }
    for i in 1..=8 {
        let guard = if i == 1 {
            "(1)".to_string()
        } else {
            format!("(done_t{} == 1)", i - 1)
        };
        s.push_str(&format!(
            "active proctype T{i}() {{\n\tdo\n\t:: {guard} ->\n\t\tactive_t{i} = 1;\n\t\tactive_t{i} = 0;\n\t\tif\n\t\t:: failed_t{i} = 1;\n\t\t:: skip;\n\t\tfi;\n\t\tdone_t{i} = 1;\n\t\tbreak\n\tod\n}}\n"
        ));
    }
    // The claim names only done_t8, so every failed_* and every active_* is invisible to it.
    s.push_str("\nltl p0 { <> ( done_t8 == 1 ) }\n");

    let result = crate::verify_with_config(
        &s,
        &crate::engine::checker::CheckerConfig {
            max_states: 500_000,
            ..Default::default()
        },
    )
    .expect("the model must run");
    // A capped run is a ceiling, not a measurement - the same trap as a vacuous threshold.
    assert!(
        !result.unverified.iter().any(|u| matches!(
            u.kind,
            crate::engine::checker::UnverifiedKind::StateBudget
                | crate::engine::checker::UnverifiedKind::DepthBudget
        )),
        "the run hit a budget, so its count is not a measurement: {:?}",
        result
            .unverified
            .iter()
            .map(|u| u.reason.clone())
            .collect::<Vec<_>>()
    );
    assert!(
        result.states_explored < 10_000,
        "a liveness claim that cannot read the failure bits explored {} states; without the \
         projection the same fixture needs ~240000",
        result.states_explored
    );
    assert_eq!(result.errors, 0, "the claim holds");
}

/// Projection must not lose a liveness violation, on the shapes where reduction is unsound.
///
/// Each of these is a way a naive projection or reduction could hide an accepting cycle. The
/// expected verdicts are pan's, measured.
#[test]
fn liveness_projection_preserves_every_verdict() {
    let cases: &[(&str, &str, usize)] = &[
        (
            "violated diamond: x never becomes 1",
            "bool x = 0;\nactive proctype P() { do :: (1) -> skip od }\nltl p0 { <> ( x == 1 ) }\n",
            1,
        ),
        (
            "satisfied diamond: x becomes 1",
            "bool x = 0;\nactive proctype P() { x = 1; do :: (1) -> skip od }\nltl p0 { <> ( x == 1 ) }\n",
            0,
        ),
        (
            "invisible cycle in another process",
            "bool f = 0;\nbool x = 0;\nactive proctype A() { do :: (1) -> f = (f+1)%2 od }\nactive proctype B() { do :: (1) -> skip od }\nltl p0 { <> ( x == 1 ) }\n",
            1,
        ),
        (
            "violation reached through a guard on an unread bit",
            "bool f = 0;\nbool bad = 0;\nactive proctype A() { do :: (1) -> f = 1 od }\nactive proctype B() { do :: (f==1) -> bad = 1; break od }\nltl p0 { <> ( bad == 1 ) }\n",
            1,
        ),
    ];
    for (what, source, want) in cases {
        let result = crate::verify(source).unwrap_or_else(|e| panic!("{what}: {e}"));
        assert_eq!(
            result.errors,
            *want,
            "{what}: expected {want} error(s), got {} ({:?})",
            result.errors,
            result
                .violations
                .iter()
                .map(|v| v.description.clone())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn bench_split_parse_vs_lua() {
    use crate::engine::checker::Model as _;
    let src = std::fs::read_to_string("/tmp/c60.pml").unwrap();
    let m = crate::LuaModel::from_source(&src).unwrap();
    let init = m.init_states();
    let blob = m.state_to_string(&init[0]).unwrap();
    let fields = blob.matches(',').count() + 1;
    let iters = 2000;
    let t = std::time::Instant::now();
    for _ in 0..iters {
        let _ = m.transitions(&init[0]);
    }
    let full = t.elapsed().as_secs_f64() / iters as f64 * 1e6;
    let vis: std::collections::BTreeSet<String> = init[0]
        .0
        .split(',')
        .map(|e| {
            e.split(':')
                .next()
                .unwrap_or("")
                .trim()
                .trim_matches('"')
                .to_string()
        })
        .collect();
    let t = std::time::Instant::now();
    for _ in 0..iters {
        let _ = m.project(&init[0], &vis);
    }
    let proj = t.elapsed().as_secs_f64() / iters as f64 * 1e6;
    let t = std::time::Instant::now();
    for _ in 0..iters {
        let _ = m.hash(&init[0]);
    }
    let h = t.elapsed().as_secs_f64() / iters as f64 * 1e6;
    eprintln!("SPLIT fields={fields} transitions={full:.1}us project={proj:.1}us hash={h:.1}us");
}

// ─── Batched expansion preserves guard semantics ───────────────────────────
//
// _spin_expand evaluates guards inside Lua instead of crossing back into Rust per transition.
// The guard rules are easy to get subtly wrong when they move (a guard is enabled when non-zero,
// nil is disabled, a boolean is taken as itself), so they are pinned here against pan's verdicts.

#[test]
fn batched_expansion_matches_pan_on_guard_shapes() {
    let cases: &[(&str, &str, usize)] = &[
        (
            "guard 0 is disabled: the process deadlocks",
            "byte x = 0;\nactive proctype P() { do :: (x > 0) -> x = 5 od }",
            1,
        ),
        (
            "a non-zero guard enables the branch",
            "byte x = 1;\nactive proctype P() { do :: (x == 1) -> x = 2; break od }",
            0,
        ),
        (
            "a comparison guard on a variable that changes",
            "byte x = 1;\nactive proctype P() { do :: (x == 1) -> x = 0; break od }",
            0,
        ),
        (
            "a boolean guard that is false deadlocks",
            "bool f = false;\nactive proctype P() { do :: (f) -> f = true; break od }",
            1,
        ),
        (
            "if/else falls through to the else branch",
            "byte x = 0;\nactive proctype P() { if :: (x > 0) -> x = 9 :: else -> x = 1 fi }",
            0,
        ),
        (
            "an assertion still fires",
            "byte x = 0;\nactive proctype P() { x = 1; assert(x == 0) }",
            1,
        ),
    ];
    for (what, source, want) in cases {
        let result = crate::verify(source).unwrap_or_else(|e| panic!("{what}: {e}"));
        assert_eq!(
            result.errors,
            *want,
            "{what}: expected {want} error(s), got {} ({:?})",
            result.errors,
            result
                .violations
                .iter()
                .map(|v| v.description.clone())
                .collect::<Vec<_>>()
        );
    }
}
