//! Evaluating a claim atoms against a state.
//!
//! Split out of the DFS so the traversal file stays a traversal: these answer whether a
//! formula is a safety property, and what an atom means in a given state.

use super::Model;
use crate::property::LtlFormula;

/// Check if an LTL formula is purely propositional (no temporal operators).
pub(super) fn is_propositional(formula: &LtlFormula) -> bool {
    match formula {
        LtlFormula::True | LtlFormula::False => true,
        LtlFormula::Atom(_) => true,
        LtlFormula::Not(inner) => is_propositional(inner),
        LtlFormula::And(l, r) | LtlFormula::Or(l, r) | LtlFormula::Implies(l, r) => {
            is_propositional(l) && is_propositional(r)
        }
        // Temporal operators
        LtlFormula::Always(_)
        | LtlFormula::Eventually(_)
        | LtlFormula::Next(_)
        | LtlFormula::Until(_, _)
        | LtlFormula::Release(_, _) => false,
    }
}

/// The first atom of `formula` that neither the state's fields nor the model can decide.
///
/// Used only to say *why* a propositional check came back undecided: a fragment the parser
/// produced and an atom the state does not carry are different debts.
pub(super) fn first_unresolved_atom<M: Model>(
    formula: &LtlFormula,
    state_str: &str,
    model: &M,
    state: &M::State,
) -> Option<String> {
    use LtlFormula as F;
    match formula {
        F::Atom(name) => {
            let resolvable = extract_var_from_state(state_str, name).is_some()
                || model.eval_atom(state, name).is_some();
            (!resolvable).then(|| name.clone())
        }
        F::True | F::False => None,
        F::Not(inner) | F::Always(inner) | F::Eventually(inner) | F::Next(inner) => {
            first_unresolved_atom(inner, state_str, model, state)
        }
        F::And(l, r) | F::Or(l, r) | F::Implies(l, r) | F::Until(l, r) | F::Release(l, r) => {
            first_unresolved_atom(l, state_str, model, state)
                .or_else(|| first_unresolved_atom(r, state_str, model, state))
        }
    }
}

/// Evaluate a propositional LTL formula against a state string.
///
/// Returns `None` when an atom does not resolve to a variable in the state.
/// An unresolved atom must never be silently read as `false`: with a property of
/// the shape `[](a -> b)` that makes the whole property vacuous and reports success
/// for a model the checker never actually judged.
pub(super) fn evaluate_propositional<M: Model>(
    formula: &LtlFormula,
    state_str: &str,
    model: &M,
    state: &M::State,
) -> Option<bool> {
    match formula {
        LtlFormula::True => Some(true),
        LtlFormula::False => Some(false),
        LtlFormula::Atom(name) => extract_var_from_state(state_str, name)
            .map(|val| val != 0)
            // A bare name is answered from the state's fields. Anything else - a comparison, an
            // arithmetic atom - is an expression, and the model may be able to decide it. Leaving
            // those undecided made every [] ( x <= 2 ) a decline.
            .or_else(|| model.eval_atom(state, name)),
        LtlFormula::Not(inner) => {
            evaluate_propositional(inner, state_str, model, state).map(|b| !b)
        }
        LtlFormula::And(l, r) => match (
            evaluate_propositional(l, state_str, model, state),
            evaluate_propositional(r, state_str, model, state),
        ) {
            (Some(a), Some(b)) => Some(a && b),
            _ => None,
        },
        LtlFormula::Or(l, r) => match (
            evaluate_propositional(l, state_str, model, state),
            evaluate_propositional(r, state_str, model, state),
        ) {
            (Some(a), Some(b)) => Some(a || b),
            _ => None,
        },
        LtlFormula::Implies(l, r) => match (
            evaluate_propositional(l, state_str, model, state),
            evaluate_propositional(r, state_str, model, state),
        ) {
            (Some(a), Some(b)) => Some(!a || b),
            _ => None,
        },
        // Temporal operators inside []expr shouldn't reach here (filtered by is_propositional)
        _ => Some(true),
    }
}

/// Check if a formula string is a safety formula ([]expr where expr is propositional).
pub(crate) fn is_safety_formula(formula_str: &str) -> bool {
    if let Ok(LtlFormula::Always(inner)) = LtlFormula::parse(formula_str) {
        return is_propositional(&inner);
    }
    false
}

/// Extract a variable's value from a state blob string.
/// State blob format: {key:val,key:val,...} or {"key":val,"key":val,...}
///
/// Returns `None` if the variable is absent, so callers can distinguish an
/// unresolved atom from a variable that is genuinely zero.
pub(crate) fn extract_var_from_state(state_str: &str, var_name: &str) -> Option<i64> {
    let inner = state_str
        .trim_start_matches('{')
        .trim_end_matches('}')
        .trim();

    // Try the quoted form first, then the unquoted form.
    for key in [format!("\"{}\":", var_name), format!("{}:", var_name)] {
        let Some(p) = inner.find(&key) else {
            continue;
        };
        // Require a key boundary so that searching for "a" cannot match the tail
        // of a longer key such as "done_a".
        let at_boundary = p == 0 || matches!(inner.as_bytes()[p - 1], b'{' | b',' | b' ');
        if !at_boundary {
            continue;
        }
        // The value starts immediately after the key's own colon. Searching for
        // another ':' here reads the next key's value instead, which made every
        // []-safety property evaluate against the wrong variable.
        let rest = inner[p + key.len()..].trim_start();
        if rest.starts_with("true") {
            return Some(1);
        }
        if rest.starts_with("false") {
            return Some(0);
        }
        if rest.starts_with("nil") {
            return Some(0);
        }
        let num: String = rest
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '-')
            .collect();
        if num.is_empty() || num == "-" {
            return None;
        }
        return num.parse::<i64>().ok();
    }
    None
}
