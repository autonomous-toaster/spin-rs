//! State layout helpers: which fields exist, which of them a claim can observe, which of them
//! decide that two states are the same, and what a model's bodies contain.
//!
//! These are static questions about a model, separate from the runtime that evaluates it. They
//! live here rather than in `runtime::mod` so that the reasoning about visibility and identity -
//! the part that is easy to get unsound - sits on its own, next to the tests that pin it.

pub(crate) const LTL_KEYWORDS: &[&str] = &[
    "true",
    "false",
    "nil",
    "U",
    "V",
    "W",
    "X",
    "skip",
    "always",
    "eventually",
    "until",
];
/// A claim observes variables; a *state* is also the position of every process in its body. The
/// generator encodes that position as `_pc_<proc>`, `_step_<proc>`, `_progress_<proc>`,
/// `_unless_<proc>` and `_done_<proc>`, plus the run-wide `_nr_pr` and `_nr_qs`.
///
/// Dropping these from state identity is not a speed/accuracy trade, it is unsound: the model
/// stops making progress, because every state after the first looks like a state already seen.
pub(crate) fn is_control_field(field: &str) -> bool {
    const PREFIXES: &[&str] = &[
        "_pc_",
        "_step_",
        "_progress_",
        "_unless_",
        "_done_",
        "_nr_pr",
        "_nr_qs",
    ];
    PREFIXES.iter().any(|p| field.starts_with(p))
}

pub(crate) fn visible_fields(
    formula: &str,
    state_fields: &[String],
) -> Option<std::collections::BTreeSet<String>> {
    let mut out = std::collections::BTreeSet::new();
    for ident in formula_identifiers(formula) {
        if LTL_KEYWORDS.contains(&ident.as_str()) {
            continue;
        }
        // A proctype local x is stored as p1_x, so a formula naming x observes every process's
        // copy. Over-approximating here costs reduction; under-approximating would be unsound.
        let suffix = format!("_{}", ident);
        let mut matched = false;
        for field in state_fields {
            if *field == ident || field.ends_with(&suffix) {
                out.insert(field.clone());
                matched = true;
            }
        }
        if !matched {
            return None;
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

/// Every identifier a formula mentions.
pub(crate) fn formula_identifiers(formula: &str) -> Vec<String> {
    let bytes = formula.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            while i < bytes.len() && {
                let c = bytes[i] as char;
                c.is_ascii_alphanumeric() || c == '_'
            } {
                i += 1;
            }
            out.push(formula[start..i].to_string());
        } else {
            i += 1;
        }
    }
    out
}

/// The state fields whose change can alter a safety claim's truth value.
///
/// Returns None when an identifier cannot be resolved to any field the state carries, so the
/// caller can fall back to treating everything as visible. Guessing in that situation - treating
/// an unresolved atom as visible-not-relevant - is how a reduced search loses a violation.
/// Fields that decide which transitions are enabled, and therefore always distinguish states.
///
pub(crate) fn stmt_has_assert(s: &crate::parser::ast::Stmt) -> bool {
    use crate::parser::ast::Stmt;
    match s {
        Stmt::Assert(_, _) => true,
        Stmt::If(gs) | Stmt::Do(gs) => gs.iter().any(|g| stmts_have_assert(&g.body)),
        // Block joins Atomic and DStep here for the same reason it joined them in
        // vars_read_by_stmt: a bare block is an ordinary container, and skipping it makes an
        // assert inside one invisible to the whole engine. Measured: an assert in a block gave
        // has_asserts=false, so no failure marker was written and the model accepted where pan
        // refutes.
        Stmt::Atomic(b, _) | Stmt::DStep(b, _) | Stmt::Block(b, _) => stmts_have_assert(b),
        Stmt::Unless { body, handler, .. } => stmts_have_assert(body) || stmts_have_assert(handler),
        Stmt::For {
            init, update, body, ..
        } => stmt_has_assert(init) || stmt_has_assert(update) || stmts_have_assert(body),
        _ => false,
    }
}

/// Does any statement of this body jump?
///
/// The reduction must not be applied to such a model: a jump declares only the step counter as a
/// dependency, so it looks independent of every other process, and a body that loops back through a
/// label loses the interleavings it exists to produce.
///
/// Only a `goto` counts. A label with no jump is a no-op step: it cannot close a cycle, so it cannot be
/// the cycle the opt-out exists to prevent, and refusing to reduce every model that merely names a label
/// was costing about half the wall clock of the whole Examples sweep. `ex_3c.pml` is the evidence that the
/// opt-out still does its job where it matters - 858 states and the assertion, where reduced it found 77
/// and none.
pub(crate) fn stmts_have_goto(body: &[crate::parser::ast::Stmt]) -> bool {
    let mut found = false;
    crate::codegen::walk_stmts(body, &mut |s| {
        if matches!(s, crate::parser::ast::Stmt::Goto(_, _)) {
            found = true;
        }
    });
    found
}
pub(crate) fn stmts_have_assert(stmts: &[crate::parser::ast::Stmt]) -> bool {
    stmts.iter().any(stmt_has_assert)
}

/// Field names carried by a serialized state blob.
pub(crate) fn parse_state_keys(blob: &str) -> Vec<String> {
    let inner = blob.trim_start_matches('{').trim_end_matches('}');
    let mut out = Vec::new();
    for entry in inner.split(',') {
        let entry = entry.trim();
        if let Some(colon) = entry.find(':') {
            let key = entry[..colon].trim().trim_matches('"');
            if !key.is_empty() {
                out.push(key.to_string());
            }
        }
    }
    out
}

// ─── Parsing helpers ────────────────────────────────────────────

/// Parse `_done_<name>` flags from a state blob to determine how many
/// processes are still running (_done == false) vs done (_done == true).
/// Returns (total_done_flags_found, running_count).
pub(crate) fn parse_done_flags(blob: &str) -> (usize, usize) {
    // State blob format: {key:val,key:val,...}
    // Look for _done_<name>:true or _done_<name>:false patterns
    let mut total = 0usize;
    let mut running = 0usize;
    let mut pos = 0;
    while let Some(start) = blob[pos..].find("_done_") {
        let actual_pos = pos + start;
        // Find the value after this key: look for ':' then check next token
        if let Some(colon_pos) = blob[actual_pos..].find(':') {
            let val_start = actual_pos + colon_pos + 1;
            // Value is true, false, or a quoted version
            let rest = &blob[val_start..];
            if rest.starts_with("true") || rest.starts_with("\"true\"") {
                total += 1;
                // Process is done
            } else if rest.starts_with("false") || rest.starts_with("\"false\"") {
                total += 1;
                running += 1;
            }
        }
        pos = actual_pos + 7; // advance past "_done_"
        if pos >= blob.len() {
            break;
        }
    }
    (total, running)
}

// ─── Convenience ────────────────────────────────────────────────

/// The model variables an `assert` expression reads.
///
/// An assertion is a finding, so the fields its condition reads MUST be visible to reduction: a
/// writer of such a field is not independent of the assert that reads it, and commuting the two
/// away loses the violation entirely.
///
/// Measured on
///   byte x = 0;
///   active proctype A() { do :: x = 1; x = 2 od }
///   active proctype B() { assert(x != 1) }
/// with no claim: pan refutes in 11 states, the reduced search accepted in 10. The always-visible
/// set held only `_failed_assert`, which is where a failure is *recorded*, not what the check
/// *reads* - so A's write of x looked independent of B's assert.
pub(crate) fn assert_reads(
    stmts: &[crate::parser::ast::Stmt],
) -> std::collections::BTreeSet<String> {
    use crate::parser::ast::Stmt;
    fn expr_vars(e: &crate::parser::ast::Expression, out: &mut std::collections::BTreeSet<String>) {
        out.extend(crate::codegen::optimize::vars_in_expr(e));
        // A compound expression's operands are covered by vars_in_expr.
    }
    fn walk(s: &Stmt, out: &mut std::collections::BTreeSet<String>) {
        match s {
            Stmt::Assert(e, _) => expr_vars(e, out),
            Stmt::If(gs) | Stmt::Do(gs) => {
                for g in gs {
                    if let Some(c) = &g.condition {
                        expr_vars(c, out);
                    }
                    for s in &g.body {
                        walk(s, out);
                    }
                }
            }
            Stmt::Atomic(b, _) | Stmt::DStep(b, _) | Stmt::Block(b, _) => {
                for s in b {
                    walk(s, out);
                }
            }
            Stmt::Unless { body, handler, .. } => {
                for s in body.iter().chain(handler.iter()) {
                    walk(s, out);
                }
            }
            Stmt::For {
                init, update, body, ..
            } => {
                walk(init, out);
                walk(update, out);
                for s in body {
                    walk(s, out);
                }
            }
            _ => {}
        }
    }
    let mut out = std::collections::BTreeSet::new();
    for s in stmts {
        walk(s, &mut out);
    }
    out
}

/// The state fields any `assert` in the model reads, across every proctype and `init`.
pub(crate) fn model_assert_reads(
    model: &crate::parser::ast::PromelaModel,
) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    for d in &model.declarations {
        match d {
            crate::parser::ast::TopLevel::Proctype(p) => out.extend(assert_reads(&p.body)),
            crate::parser::ast::TopLevel::Init(i) => out.extend(assert_reads(&i.body)),
            _ => {}
        }
    }
    out
}
