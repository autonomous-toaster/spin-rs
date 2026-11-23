//! Verdicts on the constructs where this engine chooses between implementing and refusing.
//!
//! The models come from the consumer (veriplan), which wrote them to probe exactly those
//! constructs; the expected counts are pan's, measured here with `spin -a` plus `pan -a`. They
//! live in this repository as well as theirs on purpose: theirs is the guard on what they depend
//! on, this is the guard on what this engine claims.
//!
//! The contract is deliberately narrow. A construct may be refused or declined freely - that is
//! debt, and it is counted elsewhere. What must never happen is a model that is *decided* and
//! disagrees with pan, or a claim that is refuted while also reported undecided. Each entry below
//! starts as "no verdict" and asserts a verdict the moment the construct is implemented, which is
//! when a wrong answer would otherwise arrive silently.

use crate::parity::Ours;

pub struct Construct {
    pub name: &'static str,
    /// pan's error count, measured. `None` if pan cannot build the model.
    pub pan: Option<usize>,
    pub source: &'static str,
}

pub const CONSTRUCTS: &[Construct] = &[
    Construct {
        name: "never",
        pan: Some(1),
        source: "/* A never claim that matches every run: pan reports a violation. */\nbyte x = 0;\n\nactive proctype P() {\n    x = 1\n}\n\nnever { skip }\n",
    },
    Construct {
        name: "priority_none",
        pan: Some(1),
        source: "/* The same model without the clause, as the control: this one pan refutes. */\nbool flag = false;\n\nactive proctype Hi() {\n    do\n    :: true -> skip\n    od\n}\n\nactive proctype Lo() {\n    flag = true\n}\n\nltl p0 { [] ( !flag ) }\n",
    },
    Construct {
        name: "priority",
        pan: Some(0),
        source: "/* pan runs only the highest-priority enabled process, so Lo never runs here:\n   the claim holds. Without the priority clause the claim is refuted. */\nbool flag = false;\n\nactive proctype Hi() priority 2 {\n    do\n    :: true -> skip\n    od\n}\n\nactive proctype Lo() {\n    flag = true\n}\n\nltl p0 { [] ( !flag ) }\n",
    },
    Construct {
        name: "rendezvous_holds",
        pan: Some(0),
        source: "/* A capacity-0 channel with a claim pan decides: the send and the receive must meet, so got\n   becomes true and the liveness claim holds. */\nchan c = [0] of { bit };\nbool got = false;\n\nactive proctype Sender() { c ! 1 }\nactive proctype Receiver() { c ? got }\n\nltl p0 { <> ( got ) }\n",
    },
    Construct {
        name: "rendezvous_order",
        pan: Some(0),
        source: "/* Ordering through a rendezvous: the flag is only set after the handshake, so pan can decide\n   a claim about what must be true before it. */\nchan c = [0] of { bit };\nbool handshake = false;\nbool after = false;\n\nactive proctype Sender() { c ! 1; after = true }\nactive proctype Receiver() { c ? handshake }\n\nltl p0 { [] ( after -> handshake ) }\n",
    },
    Construct {
        name: "rendezvous_refutes",
        pan: Some(1),
        source: "/* The same model with the opposite claim: pan refutes, because got does become true. */\nchan c = [0] of { bit };\nbool got = false;\n\nactive proctype Sender() { c ! 1 }\nactive proctype Receiver() { c ? got }\n\nltl p0 { [] ( !got ) }\n",
    },
    Construct {
        name: "rendezvous",
        pan: Some(0),
        source: "/* Capacity-0 channel: send and receive must meet. */\nchan c = [0] of { bit };\nbool got = false;\n\nactive proctype Sender() { c ! 1 }\nactive proctype Receiver() { c ? got }\n\nltl p0 { [] ( got -> got ) }\n",
    },
    Construct {
        name: "run",
        pan: Some(0),
        source: "byte x = 0;\n\nproctype Worker() { x = 1 }\n\nactive proctype Main() {\n    run Worker();\n    skip\n}\n\nltl p0 { [] ( x <= 1 ) }\n",
    },
    Construct {
        name: "unless_refutes",
        pan: Some(1),
        source: "/* Same escape as unless.pml, but the claim is true only up to 2: the escape lets x reach 3,\n   so pan refutes. Together with unless.pml this brackets what the escape means. */\nbool stop = false;\nbyte x = 0;\n\nactive proctype P() {\n    do\n    :: x < 5 -> x++\n    od unless (x == 3)\n}\n\nltl p0 { [] ( x <= 2 ) }\n",
    },
    Construct {
        name: "unless",
        pan: Some(0),
        source: "/* The escape fires at x == 3, so the loop cannot reach 5. */\nbool stop = false;\nbyte x = 0;\n\nactive proctype P() {\n    do\n    :: x < 5 -> x++\n    od unless (x == 3)\n}\n\nltl p0 { [] ( x <= 4 ) }\n",
    },
    // Reconstructed from the consumer's description of the pair that brackets what an
    // escape means: the handler is armed by the first statement of the body, so a correct
    // encoding leaves before the second statement runs. The control is the same model with
    // the escape checked only at the loop top, which pan refutes.
    Construct {
        name: "unless_midbody",
        pan: Some(0),
        source: "byte x = 0;\nactive proctype P() {\n    do :: x < 5 -> x = 1; x = x + 10 od unless (x == 1)\n}\nltl p0 { [] ( x <= 1 ) }\n",
    },
    Construct {
        name: "unless_midbody_toptop",
        pan: Some(1),
        source: "byte x = 0;\nactive proctype P() {\n    do :: (x < 5 && x != 1) -> x = 1; x = x + 10 od\n}\nltl p0 { [] ( x <= 1 ) }\n",
    },
    // The second half of the claim pair, which is what separates an acceptance condition from a
    // presence check: a check that reports a violation whenever a claim exists answers 1 for both.
    Construct {
        name: "never_unreachable",
        pan: Some(0),
        source: "/* The guard never holds, so the claim never completes: pan reports no error. */\nbyte x = 0;\n\nactive proctype P() {\n    x = 1\n}\n\nnever { (x == 7) }\n",
    },
    // The pending pair: spin's own automaton for `<> (got)`, embedded. Refused today (it carries
    // an `accept` label), so these rows assert nothing yet - and they are the ones that will say
    // whether accept labels are encoded at all.
    //
    // Worth recording what they actually test, because it is not what the name suggests: pan
    // reports "assertion violated" on the refuting model. The `accept_all` state sits behind an
    // infinite `do ... od` and is unreachable, so the verdict comes from the claim's own assert,
    // not from an accepting cycle - which is why these two do not settle accept-cycle support on
    // their own.
    Construct {
        name: "never_accept_holds",
        pan: Some(0),
        source: "/* The property holds: got is never set, so the claim's only move is its self-loop. */\nbool got = false;\nactive proctype P() { skip }\nnever  {    /* <> (got) */\nT0_init:\n\tdo\n\t:: atomic { ((got)) -> assert(!((got))) }\n\t:: (1) -> goto T0_init\n\tod;\naccept_all:\n\tskip\n}\n",
    },
    Construct {
        name: "never_accept_refutes",
        pan: Some(1),
        source: "/* The rendezvous handshake sets got, so the claim reaches its assert. */\nchan c = [0] of { bit };\nbool got = false;\nactive proctype Sender() { c ! 1 }\nactive proctype Receiver() { c ? got }\nnever  {    /* <> (got) */\nT0_init:\n\tdo\n\t:: atomic { ((got)) -> assert(!((got))) }\n\t:: (1) -> goto T0_init\n\tod;\naccept_all:\n\tskip\n}\n",
    },
    // The cycle pair: the accept state is reachable and spins, so the claim has an accepting cycle
    // even though it never completes and its asserts never fire. The oracle is `pan -a` - without
    // it pan reports 0 for both files, and someone checking with plain pan would conclude the pair
    // is vacuous.
    Construct {
        name: "never_cycle_accept",
        pan: Some(1),
        source: "/* The claim spins in an accept-labelled state forever: pan -a reports 1. */\nbyte x = 0;\nactive proctype P() { x = 1 }\nnever { accept_x: do :: (1) -> goto accept_x od }\n",
    },
    Construct {
        name: "never_cycle_unreachable",
        pan: Some(0),
        source: "/* The accept state is unreachable, so there is no accepting cycle: pan -a reports 0. */\nbyte x = 0;\nactive proctype P() { x = 1 }\nnever { do :: (0) -> skip od; accept_x: skip }\n",
    },
    Construct {
        name: "never_accept_unreachable_branch",
        pan: Some(0),
        source: "/* An accept-labelled state named only by a branch no reachable state can satisfy. pan -a 0:\n   the edge exists, so a translation that reads a branch body instead of its condition makes the\n   accept state reachable and refutes a claim pan accepts. */\nbyte x = 0;\nactive proctype P() { x = 1 }\nnever {\nT0:\n    do\n    :: (x == 7) -> goto accept_x\n    :: (1)     -> goto T0\n    od;\naccept_x:\n    skip\n}\n",
    },
    // The four boolean rows from the consumer's table: Promela has no boolean type, so `false` is
    // the integer 0 and `true` is 1, on initialisation and on assignment.
    Construct {
        name: "assert_not_bool_false",
        pan: Some(0),
        source: "bool flag = false;\nactive proctype P() { assert(!flag) }\n",
    },
    Construct {
        name: "assert_bool_equals_zero",
        pan: Some(0),
        source: "bool flag = false;\nactive proctype P() { assert(flag == 0) }\n",
    },
    Construct {
        name: "assign_bool_literal",
        pan: Some(1),
        source: "bool flag = false;\nactive proctype P() { flag = true }\nltl p0 { [] ( !flag ) }\n",
    },
    Construct {
        name: "assign_bool_to_bool",
        pan: Some(0),
        source: "bool a = true;\nbool b = false;\nactive proctype P() { b = a; assert(b == 1) }\n",
    },
    // The consumer's two random-receive rows, verbatim. Both need `pan -a`: without it pan reports 0
    // for both files, so a table built from plain pan records the same expectation twice and the pair
    // stops discriminating.
    Construct {
        name: "poll_rendezvous",
        pan: Some(0),
        source: "chan c = [0] of { bit };\nbyte got = 0;\nactive proctype Sender() { c ! 1 }\nactive proctype Receiver() { c ?? 1; got = 1 }\nltl p0 { <> ( got == 1 ) }\n",
    },
    Construct {
        name: "poll_no_sender",
        pan: Some(1),
        source: "chan c = [0] of { bit };\nbyte got = 0;\nactive proctype Receiver() { c ?? 1; got = 1 }\nltl p0 { <> ( got == 1 ) }\n",
    },
    // A handler of two statements: the second statement sets y = 9, which refutes the claim. An
    // engine that ran only the first statement would answer 0 against pan's 1.
    Construct {
        name: "unless_multi_handler",
        pan: Some(1),
        source: "byte x = 0;\nbyte y = 0;\nactive proctype P() { { x = 1; y = 2 } unless { (x == 1); y = 9 } }\nltl p0 { [] ( y <= 2 ) }\n",
    },
];

/// What this engine answered for one construct.
pub fn outcome(c: &Construct) -> (Ours, bool) {
    match crate::runtime::verify(c.source) {
        Ok(r) if r.unverified.is_empty() => (Ours::Verdict(r.errors), false),
        Ok(r) => {
            // Refuted *and* undecided is the one combination the result type exists to prevent.
            let both = r.violations.iter().any(|v| {
                r.unverified
                    .iter()
                    .any(|u| u.property_name == v.property_name)
            });
            (Ours::Decline, both)
        }
        Err(_) => (Ours::Refuse, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_construct_is_decided_against_pan() {
        let mut decided = 0;
        let mut undecided = 0;
        for c in CONSTRUCTS {
            let Some(pan) = c.pan else {
                continue;
            };
            let (ours, refuted_while_declining) = outcome(c);
            assert!(
                !refuted_while_declining,
                "{}: a claim must not be refuted and reported undecided at once",
                c.name
            );
            match ours {
                Ours::Verdict(n) => {
                    decided += 1;
                    assert_eq!(
                        n, pan,
                        "{}: decided the model and disagreed with pan (ours {} vs pan {})",
                        c.name, n, pan
                    );
                }
                Ours::Decline | Ours::Refuse => undecided += 1,
            }
        }
        // If every construct is refused the test above proves nothing, and a change that silently
        // stopped refusing would pass it. Some must be decided.
        assert!(
            decided > 0,
            "the table must decide at least one construct, or it is not measuring verdicts"
        );
        assert_eq!(decided + undecided, CONSTRUCTS.len());
    }

    #[test]
    fn a_refused_construct_is_refused_by_name() {
        // The README claims these are refused by name rather than by a parse error carrying an
        // offset, which a caller cannot tell apart from a malformed model of their own.
        // `unless` and `priority` used to be in this list; both moved to being decided when their
        // encodings landed, which is this table doing what it was built for.
        let c = CONSTRUCTS
            .iter()
            .find(|c| c.name == "run")
            .expect("run must be in the table");
        let err = crate::runtime::verify(c.source)
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default();
        assert!(
            err.contains("run is not supported"),
            "run must be refused by name, got: {err}"
        );
    }
}
