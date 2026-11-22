//! The generator has to be able to reproduce the defects it exists to catch.
//!
//! A parity gate built on a grammar that cannot express the shapes already known to be wrong
//! measures nothing. These tests are hermetic - no spin, no gcc - so they run in the ordinary
//! library gate, and they fail if a template is edited into something that no longer has the
//! structure the defect lived in.

use super::*;

fn find(name: &str) -> Model {
    corpus()
        .into_iter()
        .find(|m| m.name == name)
        .unwrap_or_else(|| panic!("corpus is missing {name}"))
}

#[test]
fn names_are_unique() {
    let c = corpus();
    let mut seen = std::collections::BTreeSet::new();
    for m in &c {
        assert!(
            seen.insert(m.name.clone()),
            "duplicate model name {}",
            m.name
        );
    }
}

#[test]
fn every_model_carries_at_most_one_claim() {
    for m in corpus() {
        let n = m.source.matches("ltl ").count();
        assert!(
            n <= 1,
            "{} carries {n} claims; pan checks one per run",
            m.name
        );
    }
}

#[test]
fn every_known_wrong_model_exists_and_is_recorded() {
    let c = corpus();
    for (name, _) in KNOWN_WRONG {
        assert!(
            c.iter().any(|m| m.name == *name),
            "{name} is listed as known wrong but the generator no longer produces it"
        );
        assert!(
            RECORDED_PAN.iter().any(|(n, _)| n == name),
            "{name} is known wrong but pan's verdict on it was never recorded"
        );
    }
}

#[test]
fn two_statements_are_reproduced_in_order() {
    // The shape behind the false refutation: b is written first, so the claim must hold.
    let m = find("seq2_ba__impl_ab");
    assert!(m.source.contains("b = 1; a = 1"), "{}", m.source);
    assert!(
        !m.source.contains("do ::"),
        "the shape must have no loop: {}",
        m.source
    );
    assert!(m.source.contains("[] ( a -> b )"), "{}", m.source);
}

#[test]
fn an_assignment_before_a_loop_is_reproduced() {
    let m = find("assign_then_do__impl_ab");
    let b = m.source.find("b = 1").expect("the leading write");
    let d = m.source.find("do ::").expect("the loop");
    assert!(b < d, "the write must precede the loop: {}", m.source);
}

#[test]
fn two_sequential_loops_are_reproduced() {
    let m = find("do_then_do__impl_ab");
    assert_eq!(m.source.matches("do ::").count(), 2, "{}", m.source);
    let first = m.source.find("b = 1").expect("phase one");
    let second = m.source.find("a = 1").expect("phase two");
    assert!(first < second, "{}", m.source);
}

#[test]
fn an_assert_in_a_blocked_process_is_reproduced() {
    let m = find("loop_and_assert");
    assert_eq!(
        m.source.matches("active proctype").count(),
        2,
        "{}",
        m.source
    );
    assert!(m.source.contains("assert(false)"), "{}", m.source);
    let a = m
        .source
        .find("active proctype A")
        .expect("the live process");
    let b = m
        .source
        .find("active proctype B")
        .expect("the asserting process");
    assert!(
        a < b,
        "the live process must be declared first: {}",
        m.source
    );
}

#[test]
fn the_grammar_covers_every_construct_axis() {
    // Each of these is a construct whose sequencing lives in its own counter today. If the grammar
    // stops producing one, the rewrite in Phase 2 loses its oracle for it.
    let c = corpus();
    let has = |needle: &str| c.iter().any(|m| m.source.contains(needle));
    for needle in [
        "do ::", "if ::", "atomic {", "d_step {", "for (", "goto ", "c ! 1", "unless",
    ] {
        if needle == "unless" {
            // Kept out of the generated corpus until its syntax is settled; Phase 3 owns it.
            continue;
        }
        assert!(has(needle), "no generated model contains {needle}");
    }
}

#[test]
fn every_claim_shape_is_exercised() {
    let c = corpus();
    for claim in ["[] ( a -> b )", "[] ( b -> a )", "[] ( !(a) )", "<> ( a )"] {
        assert!(
            c.iter().any(|m| m.source.contains(claim)),
            "no generated model carries the claim {claim}"
        );
    }
}

#[test]
fn classify_separates_the_six_outcomes() {
    assert_eq!(classify(Ours::Verdict(0), Some(0)), Outcome::Agree);
    assert_eq!(classify(Ours::Verdict(1), Some(1)), Outcome::Agree);
    assert_eq!(
        classify(Ours::Verdict(1), Some(0)),
        Outcome::FalseRefutation
    );
    assert_eq!(classify(Ours::Verdict(0), Some(1)), Outcome::FalseClean);
    assert_eq!(classify(Ours::Decline, Some(1)), Outcome::Decline);
    assert_eq!(classify(Ours::Refuse, Some(1)), Outcome::Refusal);
    assert_eq!(classify(Ours::Verdict(0), None), Outcome::PanUnbuildable);
    assert_eq!(classify(Ours::Decline, None), Outcome::PanUnbuildable);
}

#[test]
fn a_decline_never_counts_as_agreement() {
    // The instrument bug this change exists to stop repeating: a model we do not decide reported
    // agreement whenever our error count happened to be zero.
    for pan in [Some(0), Some(1), Some(7)] {
        assert_ne!(classify(Ours::Decline, pan), Outcome::Agree);
        assert_ne!(classify(Ours::Decline, pan), Outcome::FalseClean);
    }
}

#[test]
fn the_corpus_is_large_enough_to_be_a_grammar() {
    let c = corpus();
    assert!(
        c.len() >= TEMPLATES.len() * CLAIMS.len(),
        "every template must be crossed with every claim"
    );
    assert!(c.len() >= 60, "corpus is {} models", c.len());
}
