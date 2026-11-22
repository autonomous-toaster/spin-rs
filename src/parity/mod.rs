//! Differential corpus for parity with pan.
//!
//! The corpus is generated from a grammar over *body structure*, not over language features.
//! A curated list measures the curator: every model in the hand-written corpus had exactly one
//! statement in its proctype body, which is why a defect that reorders two statements survived a
//! "74 of 75 agree" measurement.
//!
//! The generator is used two ways:
//!
//! - `cargo run --release --bin parity_gate -- --baseline` records what the current binary
//!   does against pan.
//! - `cargo run --release --bin parity_gate -- --gate` fails on a wrong verdict that is not
//!   already listed in `KNOWN_WRONG`, and warns when a listed one starts passing. That is
//!   what keeps the list from rotting.

#[cfg(test)]
mod tests;

/// One generated model: a Promela source with exactly one claim.
#[derive(Debug, Clone)]
pub struct Model {
    /// Stable identifier, used as the report key and in the known-wrong list.
    pub name: String,
    /// The Promela source.
    pub source: String,
    /// The template and claim that produced it, for grouping a report.
    pub template: &'static str,
    pub claim: &'static str,
}

struct Template {
    name: &'static str,
    globals: &'static str,
    body: &'static str,
}

/// Body shapes. Each one is a way a program can order, nest or guard two writes: `b` before
/// `a` means the claim `[] ( a -> b )` should hold, and any engine that reorders them refutes it.
const TEMPLATES: &[Template] = &[
    Template {
        name: "seq2_ba",
        globals: "",
        body: "b = 1; a = 1",
    },
    Template {
        name: "seq2_ab",
        globals: "",
        body: "a = 1; b = 1",
    },
    Template {
        name: "seq3_bxa",
        globals: "",
        body: "b = 1; skip; a = 1",
    },
    Template {
        name: "assign_then_do",
        globals: "",
        body: "b = 1; do :: (1) -> a = 1; break od",
    },
    Template {
        name: "do_then_assign",
        globals: "",
        body: "do :: (1) -> b = 1; break od; a = 1",
    },
    Template {
        name: "do_then_do",
        globals: "",
        body: "do :: (1) -> b = 1; break od; do :: (1) -> a = 1; break od",
    },
    Template {
        name: "do_two_stmts",
        globals: "",
        body: "do :: (1) -> b = 1; a = 1; break od",
    },
    Template {
        name: "if_then_assign",
        globals: "",
        body: "if :: (1) -> b = 1 fi; a = 1",
    },
    Template {
        name: "nested_if_two",
        globals: "",
        body: "if :: (1) -> if :: (1) -> b = 1 fi fi; a = 1",
    },
    Template {
        name: "nested_if_multi",
        globals: "",
        body: "if :: (1) -> if :: (1) -> b = 1; a = 1 fi fi",
    },
    Template {
        name: "atomic_region",
        globals: "",
        body: "atomic { b = 1; a = 1 }",
    },
    Template {
        name: "assign_then_atomic",
        globals: "",
        body: "b = 1; atomic { a = 1 }",
    },
    Template {
        name: "dstep_region",
        globals: "",
        body: "d_step { b = 1; a = 1 }",
    },
    Template {
        name: "for_then_assign",
        globals: "",
        body: "byte i; for (i : 0 .. 0) { b = 1 }; a = 1",
    },
    // A `for` whose body has two statements. The body is not atomic: pan makes the state after
    // the first write observable, so `[] ( c -> b )` is refuted here. Expanding the loop to an
    // atomic region at parse time hid exactly that, and the single-iteration template below
    // could not see it.
    Template {
        name: "for_two_stmts",
        globals: "",
        body: "byte i; for (i : 0 .. 1) { d = 1; b = 1 }",
    },
    Template {
        name: "for_multi_then_assign",
        globals: "",
        body: "byte i; for (i : 0 .. 2) { d = 1 }; b = 1; a = 1",
    },
    Template {
        name: "goto_over",
        globals: "",
        body: "b = 1; goto L; a = 1; L: skip",
    },
    Template {
        name: "send_then_assign",
        globals: "chan c = [1] of { bit };",
        body: "c ! 1; b = 1; a = 1",
    },
];

struct Claim {
    name: &'static str,
    text: &'static str,
}

/// Claims. One per model, because pan checks one claim per run and the comparison has to be like
/// for like.
const CLAIMS: &[Claim] = &[
    Claim {
        name: "impl_ab",
        text: "[] ( a -> b )",
    },
    Claim {
        name: "impl_ba",
        text: "[] ( b -> a )",
    },
    Claim {
        name: "not_a",
        text: "[] ( !(a) )",
    },
    // Observes an intermediate state of a two-statement region: only a region that is genuinely
    // one transition can hold it.
    Claim {
        name: "impl_db",
        text: "[] ( d -> b )",
    },
    // A comparison atom, which is an expression over two fields rather than one name. It used to
    // be undecidable - reported as unchecked, never as a verdict - so no template could exercise
    // it.
    Claim {
        name: "eq_ab",
        text: "[] ( a == b )",
    },
    Claim {
        name: "ev_a",
        text: "<> ( a )",
    },
];

/// Declarations shared by every single-process shape.
// `d` rather than `c`: the channel template declares a channel named `c`, and redeclaring it
// makes pan refuse the model.
const DECLS: &str = "bool a, b, d;";

fn render(globals: &str, procs: &str, claim: Option<&str>) -> String {
    let mut out = String::new();
    out.push_str(DECLS);
    out.push('\n');
    if !globals.is_empty() {
        out.push_str(globals);
        out.push('\n');
    }
    out.push_str(procs);
    out.push('\n');
    if let Some(c) = claim {
        out.push_str("ltl p { ");
        out.push_str(c);
        out.push_str(" }\n");
    }
    out
}

/// Two processes where one never blocks, so the other's assert is never the reason the model stops.
///
/// This is the shape behind the false clean: a failed assertion that only disables its own
/// transition is invisible while any other process keeps the system live.
fn loop_and_assert() -> Model {
    Model {
        name: "loop_and_assert".to_string(),
        source: render(
            "",
            "active proctype A() { do :: (1) -> skip od }\n\
             active proctype B() { do :: (1) -> assert(false); break od }",
            None,
        ),
        template: "loop_and_assert",
        claim: "model",
    }
}

/// Two-process shapes.
///
/// Every entry in `TEMPLATES` is a single process, so no transition of one process ever races
/// another and reduction is never exercised. These pair the same bodies with a second process
/// that writes the same variables: whether the verdict is a refutation or not, pan and we have
/// to agree on it, and a reduction that dropped an interfering transition would disagree.
fn two_process_models() -> Vec<Model> {
    const SHAPES: &[(&str, &str, &str)] = &[
        // A second writer of `a`: `[] ( a -> b )` is genuinely refuted, because Q sets a while
        // P has not reached b yet.
        ("race_a", "b = 1; a = 1", "a = 1"),
        // A second process that reads `a` in a guard: the guard can only fire after P sets it.
        (
            "guard_a",
            "b = 1; a = 1",
            "if :: (a) -> d = 1 :: else -> skip fi",
        ),
        // A rendezvous channel: its send pairs with a receive in another process, so it is one
        // transition over two processes. This engine moves one process per transition, so the
        // shape is counted as a refusal rather than left out of the corpus.
        ("rendezvous", "c ! 1; b = 1", "bit x; c ? x"),
    ];
    let mut out = Vec::new();
    for (name, p_body, q_body) in SHAPES {
        for c in CLAIMS {
            let procs =
                format!("active proctype P() {{ {p_body} }}\nactive proctype Q() {{ {q_body} }}");
            // The rendezvous shape needs a capacity-0 declaration; every other shape uses the
            // shared buffered declarations, if any.
            let globals = if *name == "rendezvous" {
                "chan c = [0] of { bit };"
            } else {
                ""
            };
            out.push(Model {
                name: format!("{name}__{}", c.name),
                source: render(globals, &procs, Some(c.text)),
                template: name,
                claim: c.name,
            });
        }
    }
    out
}

/// Shapes that each exist because a defect was found by hand and must not come back.
///
/// A generator that cannot reproduce a known defect cannot find an unknown one, and these are the
/// ones the grammar has no axis for: they need two processes, or a construct its templates do not
/// emit. Each is here because it was wrong once.
fn fixed_models() -> Vec<Model> {
    const SHAPES: &[(&str, &str, &str)] = &[
        // A channel's queue lives in the state. It used to live in the runtime, so reopening an
        // earlier state saw the messages a later one had left behind and this model - which pan
        // accepts - was refuted with an "accepting cycle" at trail length 0.
        (
            "channel_state",
            "bool a;\nchan c = [1] of { bit };\nactive proctype P() { c ! 1; a = 1 }",
            "<> ( a )",
        ),
        // A send declares the channel as written, so reduction may not order it freely against a
        // test of the same channel. Without that this model was clean with reduction on and
        // refuted with it off.
        (
            "channel_por",
            "chan c = [2] of { bit };\nbit bad = 0;\nactive proctype P() { c ! 1 }\nactive proctype Q() { if :: (len(c) == 0) -> bad = 1 :: else -> skip fi }",
            "[] ( !bad )",
        ),
        // A receive written as a guard, and the capacity of a channel array element.
        (
            "recv_guard",
            "chan c[2] = [1] of { bit };\nbit a = 0, b = 0;\nactive proctype P() { c[0] ! 1 }\nactive proctype Q() { bit x; if :: c[0] ? x -> a = 1 :: else -> skip fi }",
            "[] ( !a )",
        ),
        // A do loop whose guards run out blocks; it does not fall through. pan calls that an
        // invalid end state, and reporting it as clean would be a false green.
        (
            "do_blocks",
            "byte n = 0;\nactive proctype P() { do :: (n < 3) -> n = n + 1 od; assert(n == 3) }",
            "",
        ),
    ];
    SHAPES
        .iter()
        .map(|(name, body, claim)| Model {
            name: name.to_string(),
            source: if claim.is_empty() {
                format!("{body}\n")
            } else {
                format!("{body}\nltl p {{ {claim} }}\n")
            },
            template: name,
            claim: if claim.is_empty() { "model" } else { "p" },
        })
        .collect()
}

/// Every generated model.
pub fn corpus() -> Vec<Model> {
    let mut out = Vec::new();
    for t in TEMPLATES {
        for c in CLAIMS {
            let procs = format!("active proctype P() {{ {} }}", t.body);
            out.push(Model {
                name: format!("{}__{}", t.name, c.name),
                source: render(t.globals, &procs, Some(c.text)),
                template: t.name,
                claim: c.name,
            });
        }
    }
    out.push(loop_and_assert());
    out.extend(two_process_models());
    out.extend(fixed_models());
    out
}

/// What one engine said about one model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ours {
    /// A verdict, carrying the error count.
    Verdict(usize),
    /// No verdict: the run was incomplete and said so.
    Decline,
    /// The run failed outright, so there is no verdict to compare.
    Refuse,
}

/// The comparison of one model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Agree,
    /// We report an error pan does not: we refute a valid model.
    FalseRefutation,
    /// Pan reports an error we do not: we accept an invalid model.
    FalseClean,
    /// We declined to decide. Debt, never agreement.
    Decline,
    /// We failed to run the model at all.
    Refusal,
    /// pan could not build the model, so there is nothing to compare against.
    PanUnbuildable,
}

impl Outcome {
    pub fn label(self) -> &'static str {
        match self {
            Outcome::Agree => "agree",
            Outcome::FalseRefutation => "false refutation",
            Outcome::FalseClean => "false clean",
            Outcome::Decline => "decline",
            Outcome::Refusal => "refusal",
            Outcome::PanUnbuildable => "pan unbuildable",
        }
    }

    /// True when the comparison found a difference that is our fault.
    pub fn is_wrong(self) -> bool {
        matches!(self, Outcome::FalseRefutation | Outcome::FalseClean)
    }
}

/// Compare one model. `pan_errors` of `None` means pan could not build it.
pub fn classify(ours: Ours, pan_errors: Option<usize>) -> Outcome {
    let Some(pan) = pan_errors else {
        return Outcome::PanUnbuildable;
    };
    match ours {
        Ours::Decline => Outcome::Decline,
        Ours::Refuse => Outcome::Refusal,
        Ours::Verdict(n) if n == pan => Outcome::Agree,
        Ours::Verdict(n) if n > pan => Outcome::FalseRefutation,
        Ours::Verdict(_) => Outcome::FalseClean,
    }
}

/// Models known to be wrong today, with the outcome they are expected to keep.
///
/// The gate fails on a wrong verdict that is not listed here, and warns when a listed model starts
/// passing - so Phase 2 cannot land without emptying this list.
pub const KNOWN_WRONG: &[(&str, Outcome)] = &[
    ("seq2_ba__impl_ab", Outcome::FalseRefutation),
    ("assign_then_do__impl_ab", Outcome::FalseRefutation),
    ("do_then_do__impl_ab", Outcome::FalseRefutation),
    ("loop_and_assert", Outcome::FalseClean),
];

/// pan's verdict on the shapes that pin the known defects, recorded when they were found.
///
/// The gate re-derives these from pan and fails if the generator no longer produces a model pan
/// decides the same way. That is what stops the generator drifting away from the defects it exists
/// to catch.
pub const RECORDED_PAN: &[(&str, usize)] = &[
    ("seq2_ba__impl_ab", 0),
    ("assign_then_do__impl_ab", 0),
    ("do_then_do__impl_ab", 0),
    ("loop_and_assert", 1),
];
