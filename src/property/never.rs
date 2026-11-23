//! Translating a never claim into the automaton the product search uses.
//!
//! pan's never claim is a monitor over states: the claim sees each new state and moves when it can,
//! and a run is a violation when the claim *completes*, when an assert inside it fails, or when it
//! can cycle through an accept-labelled state. This module turns the claim body into the same kind
//! of automaton the LTL path builds, so the existing product search can run it.
//!
//! The verdict corresponds to pan -a: pan needs that flag to look for acceptance cycles, and this
//! engine always looks. That is the same convention the LTL path follows.
//!
//! Supported: guards, assert, skip, labels, goto, do/od, if/fi, atomic, and nested sequences of
//! those. Refused by name: a claim that assigns, sends or receives - that is a program, not a
//! monitor - and else guards, which need a selection over the other guards.

use crate::parser::ast::{BinaryOp, Expression, NeverClaim, Stmt, UnaryOp};

use super::ltl2ba::buchi::{BuchiAutomaton, BuchiTransition};

/// One position in a laid-out claim.
enum CNode {
    /// A guard that must hold to move on; None is always executable.
    Guard {
        text: Option<String>,
        next: usize,
    },
    /// assert(e): moving on requires e, and failing it *is* the violation.
    Assert {
        cond: String,
        next: usize,
    },
    Jump(usize),
    Goto(String),
    Select {
        options: Vec<(Option<String>, usize)>,
    },
    Halt,
    Refuse(&'static str),
}

/// Render an expression back to Promela text.
///
/// The product search decides an atom by name, so a guard travels as the source text it came from
/// and is evaluated by the same atom evaluator an LTL formula uses.
fn render(e: &Expression) -> Option<String> {
    Some(match e {
        Expression::IntLit(n) => n.to_string(),
        Expression::BoolLit(b) => {
            if *b {
                "1".to_string()
            } else {
                "0".to_string()
            }
        }
        Expression::Ident(n) => n.clone(),
        Expression::ArrayAccess { name, index } => format!("{}[{}]", name, render(index)?),
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

/// The number of positions a claim statement occupies.
fn size(s: &Stmt) -> usize {
    match s {
        Stmt::If(gs) | Stmt::Do(gs) => {
            1 + gs
                .iter()
                .map(|g| g.body.iter().map(size).sum::<usize>())
                .sum::<usize>()
        }
        Stmt::Atomic(b, _) | Stmt::DStep(b, _) => b.iter().map(size).sum::<usize>().max(1),
        _ => 1,
    }
}

struct Layout {
    nodes: Vec<CNode>,
    labels: Vec<(String, usize)>,
}

impl Layout {
    fn new(total: usize) -> Self {
        Self {
            nodes: (0..total).map(|_| CNode::Refuse("unreached")).collect(),
            labels: Vec::new(),
        }
    }

    /// Guard text of a branch: its condition, or the first statement when that is a guard.
    fn branch_guard(body: &[Stmt]) -> Result<Option<String>, String> {
        match body.first() {
            Some(Stmt::Expr(e, _)) => {
                Ok(Some(render(e).ok_or_else(|| {
                    "an expression this engine cannot render".to_string()
                })?))
            }
            _ => Ok(None),
        }
    }

    fn seq(
        &mut self,
        stmts: &[Stmt],
        base: usize,
        cont: usize,
        brk: usize,
    ) -> Result<usize, String> {
        let mut starts = Vec::new();
        let mut idx = base;
        for s in stmts {
            starts.push(idx);
            idx += size(s);
        }
        for (i, s) in stmts.iter().enumerate() {
            let next = if i + 1 < stmts.len() {
                starts[i + 1]
            } else {
                cont
            };
            self.stmt(s, starts[i], next, brk)?;
        }
        Ok(idx)
    }

    fn stmt(&mut self, s: &Stmt, at: usize, next: usize, brk: usize) -> Result<(), String> {
        match s {
            Stmt::Skip(_) => self.nodes[at] = CNode::Jump(next),
            Stmt::Expr(e, _) => {
                self.nodes[at] = CNode::Guard {
                    text: Some(
                        render(e)
                            .ok_or_else(|| "an expression this engine cannot render".to_string())?,
                    ),
                    next,
                }
            }
            Stmt::Assert(e, _) => {
                let cond =
                    render(e).ok_or_else(|| "an assert this engine cannot render".to_string())?;
                self.nodes[at] = CNode::Assert { cond, next };
            }
            Stmt::Label(name, _) => {
                self.labels.push((name.clone(), at));
                self.nodes[at] = CNode::Jump(next);
            }
            Stmt::Goto(l, _) => self.nodes[at] = CNode::Goto(l.clone()),
            Stmt::Break(_) => self.nodes[at] = CNode::Jump(brk),
            Stmt::If(gs) | Stmt::Do(gs) => {
                let is_loop = matches!(s, Stmt::Do(_));
                let mut cursor = at + 1;
                let mut options = Vec::new();
                for g in gs {
                    let start = cursor;
                    let (body_cont, body_brk) = if is_loop { (at, next) } else { (next, brk) };
                    cursor = self.seq(&g.body, start, body_cont, body_brk)?;
                    options.push((Self::branch_guard(&g.body)?, start));
                }
                self.nodes[at] = CNode::Select { options };
            }
            Stmt::Atomic(b, _) | Stmt::DStep(b, _) => {
                if b.is_empty() {
                    self.nodes[at] = CNode::Jump(next);
                } else {
                    self.seq(b, at, next, brk)?;
                }
            }
            Stmt::Unless { .. } => return Err("an unless inside a never claim".to_string()),
            _ => return Err("a claim statement that is not a monitor step".to_string()),
        }
        Ok(())
    }
}

/// Turn a never claim body into the automaton the product search runs.
pub fn claim_to_buchi(claim: &NeverClaim) -> Result<BuchiAutomaton, String> {
    let total: usize = claim.body.iter().map(size).sum::<usize>().max(1);
    let mut layout = Layout::new(total + 2);
    let halt = total;
    let violation = total + 1;
    layout.seq(&claim.body, 0, halt, halt)?;
    layout.nodes[halt] = CNode::Halt;
    // The violation state is terminal: its self-loop is emitted below, so the node table must not
    // leave it as an unfilled placeholder.
    layout.nodes[violation] = CNode::Halt;

    // spin marks a label an acceptance label by its name, which is why accept_all and accept_x both
    // count. With one present, completion is not a violation by itself: pan looks for a cycle
    // through the labelled state.
    let accept_states: Vec<usize> = layout
        .labels
        .iter()
        .filter(|(n, _)| n.starts_with("accept"))
        .map(|(_, i)| *i)
        .collect();

    let mut transitions: Vec<Vec<BuchiTransition>> = vec![Vec::new(); layout.nodes.len()];
    // The violation state is always accepting: reaching it - by completing the claim or by failing
    // an assert inside it - is the run pan reports.
    let mut accepting: std::collections::HashSet<usize> = accept_states.iter().copied().collect();
    accepting.insert(violation);

    transitions[halt].push(BuchiTransition {
        to: violation,
        conditions: vec![("true".to_string(), true)],
    });
    transitions[violation].push(BuchiTransition {
        to: violation,
        conditions: vec![("true".to_string(), true)],
    });

    for (i, node) in layout.nodes.iter().enumerate() {
        match node {
            CNode::Guard { text, next } => {
                let conditions = match text {
                    Some(t) => vec![(t.clone(), true)],
                    None => vec![("true".to_string(), true)],
                };
                transitions[i].push(BuchiTransition {
                    to: *next,
                    conditions,
                });
            }
            CNode::Assert { cond, next } => {
                transitions[i].push(BuchiTransition {
                    to: *next,
                    conditions: vec![(cond.clone(), true)],
                });
                transitions[i].push(BuchiTransition {
                    to: violation,
                    conditions: vec![(format!("!({})", cond), true)],
                });
            }
            CNode::Jump(next) => transitions[i].push(BuchiTransition {
                to: *next,
                conditions: vec![("true".to_string(), true)],
            }),
            CNode::Goto(name) => match layout.labels.iter().find(|(n, _)| n == name) {
                Some((_, target)) => transitions[i].push(BuchiTransition {
                    to: *target,
                    conditions: vec![("true".to_string(), true)],
                }),
                None => {
                    return Err(format!(
                        "a goto to a label the claim does not have: {}",
                        name
                    ));
                }
            },
            CNode::Select { options } => {
                for (guard, target) in options {
                    let conditions = match guard {
                        Some(t) => vec![(t.clone(), true)],
                        None => vec![("true".to_string(), true)],
                    };
                    transitions[i].push(BuchiTransition {
                        to: *target,
                        conditions,
                    });
                }
            }
            CNode::Halt => {}
            CNode::Refuse(why) => return Err((*why).to_string()),
        }
    }

    Ok(BuchiAutomaton {
        num_states: transitions.len(),
        initial: 0,
        accepting,
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
        assert!(a.accepting.iter().any(|s| *s != 0));
    }

    #[test]
    fn a_guard_claim_carries_its_condition() {
        let c = claim("byte x;\nnever { (x == 7) }");
        let a = claim_to_buchi(&c).expect("a guard claim is supported");
        assert_eq!(
            a.transitions[0][0].conditions,
            vec![("(x == 7)".to_string(), true)]
        );
    }

    #[test]
    fn an_accept_label_becomes_the_accepting_state() {
        let c = claim("never { accept_x: do :: (1) -> goto accept_x od }");
        let a = claim_to_buchi(&c).expect("a labelled claim is supported");
        assert!(a.accepting.contains(&0), "accepting: {:?}", a.accepting);
    }

    #[test]
    fn a_claim_that_assigns_is_refused_by_name() {
        let c = claim("byte x;\nnever { x = 1 }");
        let err = claim_to_buchi(&c).expect_err("an assigning claim is refused");
        assert!(err.contains("not a monitor step"), "{err}");
    }
}
