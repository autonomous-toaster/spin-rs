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
    Claim {
        name: "ev_a",
        text: "<> ( a )",
    },
];

/// Declarations shared by every single-process shape.
const DECLS: &str = "bool a, b;";

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
