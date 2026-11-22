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
        for (name, needle) in [
            ("priority", "priority is not implemented"),
            ("run", "run is not supported"),
            ("unless", "unless is not implemented"),
        ] {
            let c = CONSTRUCTS
                .iter()
                .find(|c| c.name == name)
                .unwrap_or_else(|| panic!("{name} must be in the table"));
            let err = crate::runtime::verify(c.source)
                .err()
                .map(|e| e.to_string())
                .unwrap_or_default();
            assert!(
                err.contains(needle),
                "{name} must be refused with '{needle}', got: {err}"
            );
        }
    }

    #[test]
    fn a_never_claim_gives_no_verdict_rather_than_a_clean_one() {
        let c = CONSTRUCTS
            .iter()
            .find(|c| c.name == "never")
            .expect("the table must carry the never model");
        let r = crate::runtime::verify(c.source).expect("the model loads");
        assert!(
            r.unverified
                .iter()
                .any(|u| u.property_name.starts_with("never")),
            "a never claim must be reported as unchecked: {:?}",
            r.unverified
        );
        assert_eq!(r.errors, 0, "an unchecked claim is not a violation");
    }
}
