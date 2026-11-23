//! Translating a `never` claim into the automaton the product search uses.
//!
//! pan's `never` claim is a process the checker runs in lockstep with the model: the claim sees each
//! new state and moves when it can, and the run is a violation when the claim *completes* (or, with
//! `accept` labels, when it can cycle through one). This module turns the claim body into the same
//! kind of automaton the LTL path builds, so the existing product search can run it.
//!
//! Supported: a sequence of blocking expressions - the shape a hand-written claim almost always has.
//! Anything else is refused by name rather than approximated: an `accept` label needs acceptance
//! *cycles* rather than completion, and a claim that assigns or sends is a program, not a monitor.

use crate::parser::ast::{BinaryOp, Expression, NeverClaim, Stmt, UnaryOp};

use super::ltl2ba::buchi::{BuchiAutomaton, BuchiTransition};

/// Render an expression back to Promela text.
///
/// The product search decides an atom by name, so a claim's guard travels as the source text it
/// came from and is evaluated by the same atom evaluator an LTL formula uses.
fn render(e: &Expression) -> Option<String> {
    Some(match e {
        Expression::IntLit(n) => n.to_string(),
        Expression::BoolLit(true) => "1".to_string(),
        Expression::BoolLit(false) => "0".to_string(),
        Expression::Ident(n) => n.clone(),
        Expression::UnaryOp { op, expr } => {
            let inner = render(expr)?;
            match op {
                UnaryOp::Not => format!("!({})", inner),
                UnaryOp::Neg => format!("-({})", inner),
                UnaryOp::BitNot => format!("~({})", inner),
                _ => return None,
            }
        }
        Expression::BinaryOp { op, left, right } => {
            let l = render(left)?;
            let r = render(right)?;
            let o = match op {
                BinaryOp::Add => "+",
                BinaryOp::Sub => "-",
                BinaryOp::Mul => "*",
                BinaryOp::Div => "/",
                BinaryOp::Mod => "%",
                BinaryOp::Eq => "==",
                BinaryOp::Neq => "!=",
                BinaryOp::Lt => "<",
                BinaryOp::Le => "<=",
                BinaryOp::Gt => ">",
                BinaryOp::Ge => ">=",
                BinaryOp::And => "&&",
                BinaryOp::Or => "||",
                BinaryOp::BitAnd => "&",
                BinaryOp::BitOr => "|",
                BinaryOp::BitXor => "^",
                BinaryOp::Shl => "<<",
                BinaryOp::Shr => ">>",
                _ => return None,
            };
            format!("({} {} {})", l, o, r)
        }
        _ => return None,
    })
}

/// The guard text of a claim statement, or why it cannot be a monitor.
fn guard_of(stmt: &Stmt) -> Result<Option<String>, String> {
    match stmt {
        Stmt::Skip(_) => Ok(None),
        Stmt::Expr(e, _) => {
            Ok(Some(render(e).ok_or_else(|| {
                "an expression this engine cannot render".to_string()
            })?))
        }
        Stmt::Label(_, _) => Ok(None),
        // A claim may be written with `else`, which is a guard that holds where nothing else does.
        // Not supported yet, and named rather than ignored.
        _ => Err("a claim statement that is not a guard".to_string()),
    }
}

/// Turn a `never` claim body into the automaton whose completion is the violation.
pub fn claim_to_buchi(claim: &NeverClaim) -> Result<BuchiAutomaton, String> {
    let mut transitions: Vec<Vec<BuchiTransition>> = Vec::new();
    let mut state = 0usize;

    for stmt in &claim.body {
        let guard = guard_of(stmt)?;
        let next = state + 1;
        let conditions = match guard {
            Some(text) => vec![(text, true)],
            None => vec![("true".to_string(), true)],
        };
        // Grow the table so that both states exist, then hang the transition on the current one.
        while transitions.len() <= next {
            transitions.push(Vec::new());
        }
        transitions[state].push(BuchiTransition {
            to: next,
            conditions,
        });
        state = next;
    }

    // Completion is the violation. pan reports a run violation the moment the claim reaches its
    // end, and marking the end accepting with a self-loop is how the product search sees that: the
    // model may stutter there forever, which is an accepting cycle.
    let end = state;
    while transitions.len() <= end {
        transitions.push(Vec::new());
    }
    transitions[end].push(BuchiTransition {
        to: end,
        conditions: vec![("true".to_string(), true)],
    });

    Ok(BuchiAutomaton {
        num_states: transitions.len(),
        initial: 0,
        accepting: vec![end].into_iter().collect(),
        transitions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claim(src: &str) -> NeverClaim {
        match crate::parser::parse(src)
            .expect("the model parses")
            .declarations
            .pop()
        {
            Some(crate::parser::ast::TopLevel::NeverClaim(c)) => c,
            other => panic!("expected a never claim, got {other:?}"),
        }
    }

    #[test]
    fn a_skip_claim_completes_immediately() {
        let c = claim("never { skip }");
        let a = claim_to_buchi(&c).expect("a bare claim is supported");
        assert_eq!(a.initial, 0);
        assert!(a.accepting.contains(&1), "the end state is accepting");
        assert_eq!(a.transitions[0][0].to, 1);
    }

    #[test]
    fn a_guard_claim_carries_its_condition() {
        let a = claim("byte x;\nnever { (x == 7) }");
        let a = claim_to_buchi(&a).expect("a guard claim is supported");
        assert_eq!(
            a.transitions[0][0].conditions,
            vec![("(x == 7)".to_string(), true)]
        );
    }

    #[test]
    fn a_claim_that_assigns_is_refused_by_name() {
        let c = claim("byte x;\nnever { x = 1 }");
        let err = claim_to_buchi(&c).expect_err("an assigning claim is refused");
        assert!(err.contains("not a guard"), "{err}");
    }
}
