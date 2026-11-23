//! Expanding `inline` calls into the body they name.
//!
//! Promela's `inline` is a macro: the body is spliced where the call appears, with the arguments
//! substituted for the parameters - textually, which is why a parameter may be used as a channel, as
//! an expression, or as the target of an assignment.
//!
//! This runs before layout, so the step machine never sees a call. It used to see one: the call was
//! emitted literally and became a call to a Lua global nothing defines, which failed at run time with
//! `attempt to call a nil value (global 'phase')` - neither a verdict nor a refusal.

use std::collections::BTreeMap;

use crate::parser::ast::{
    Expression, Guard, InlineDef, PromelaModel, RecvTarget, SendTarget, Stmt, TopLevel,
};

/// How deep inline calls may nest before the expansion is refused.
///
/// An inline that calls itself is a loop in the source, not in the model, and expanding it would not
/// terminate. Promela's own limit is not specified, so this is a guard against a hang rather than a
/// semantic: a model that reaches it is refused by name.
const MAX_DEPTH: usize = 32;

/// Replace every inline call in every body of the model with its substituted body.
pub(crate) fn expand(model: &PromelaModel) -> Result<PromelaModel, String> {
    let defs: BTreeMap<String, InlineDef> = model
        .declarations
        .iter()
        .filter_map(|d| match d {
            TopLevel::Inline(i) => Some((i.name.clone(), i.clone())),
            _ => None,
        })
        .collect();
    if defs.is_empty() {
        return Ok(model.clone());
    }
    let mut out = model.clone();
    for decl in &mut out.declarations {
        match decl {
            TopLevel::Proctype(p) => {
                p.body = expand_seq(&p.body, &defs, 0)?;
            }
            TopLevel::Init(i) => {
                i.body = expand_seq(&i.body, &defs, 0)?;
            }
            _ => {}
        }
    }
    Ok(out)
}

/// Expand calls inside a statement sequence, keeping everything else in place.
fn expand_seq(
    body: &[Stmt],
    defs: &BTreeMap<String, InlineDef>,
    depth: usize,
) -> Result<Vec<Stmt>, String> {
    if depth > MAX_DEPTH {
        return Err(format!(
            "an inline call nests more than {MAX_DEPTH} deep, which is a call cycle in the source rather than a model this engine can lay out"
        ));
    }
    let mut out = Vec::with_capacity(body.len());
    for s in body {
        if let Stmt::Expr(Expression::FuncCall { name, args }, _) = s
            && let Some(def) = defs.get(name)
        {
            if def.parameters.len() != args.len() {
                return Err(format!(
                    "inline '{}' takes {} argument(s) and is called with {}",
                    name,
                    def.parameters.len(),
                    args.len()
                ));
            }
            let subs: BTreeMap<String, Expression> = def
                .parameters
                .iter()
                .cloned()
                .zip(args.iter().cloned())
                .collect();
            let substituted = substitute_seq(&def.body, &subs)?;
            out.extend(expand_seq(&substituted, defs, depth + 1)?);
            continue;
        }
        out.push(expand_stmt(s, defs, depth)?);
    }
    Ok(out)
}

fn expand_stmt(s: &Stmt, defs: &BTreeMap<String, InlineDef>, depth: usize) -> Result<Stmt, String> {
    let mut s = s.clone();
    match &mut s {
        Stmt::If(gs) | Stmt::Do(gs) => {
            for g in gs.iter_mut() {
                g.body = expand_seq(&g.body, defs, depth)?;
            }
        }
        Stmt::Atomic(b, _) | Stmt::DStep(b, _) | Stmt::Block(b, _) => {
            *b = expand_seq(b, defs, depth)?;
        }
        Stmt::Unless { body, handler, .. } => {
            *body = expand_seq(body, defs, depth)?;
            *handler = expand_seq(handler, defs, depth)?;
        }
        Stmt::For { body, .. } => {
            *body = expand_seq(body, defs, depth)?;
        }
        _ => {}
    }
    Ok(s)
}

fn substitute_seq(body: &[Stmt], subs: &BTreeMap<String, Expression>) -> Result<Vec<Stmt>, String> {
    body.iter().map(|s| substitute_stmt(s, subs)).collect()
}

/// The identifier an lvalue parameter stands for, if the argument is a bare name.
fn lvalue_name(e: &Expression) -> Option<(String, Option<Box<Expression>>)> {
    match e {
        Expression::Ident(n) => Some((n.clone(), None)),
        Expression::ArrayAccess { name, index } => Some((name.clone(), Some(index.clone()))),
        _ => None,
    }
}

fn substitute_stmt(s: &Stmt, subs: &BTreeMap<String, Expression>) -> Result<Stmt, String> {
    let mut s = s.clone();
    match &mut s {
        Stmt::Assignment {
            target,
            index,
            value,
            ..
        } => {
            // A parameter used as the target: `inline set(x) { x = 1 }` called as `set(y)`.
            if let Some(sub) = subs.get(target.as_str()) {
                let (name, sub_index) = lvalue_name(sub).ok_or_else(|| {
                    format!(
                        "an inline parameter is used as an assignment target and bound to something that is not a name: '{}'",
                        target
                    )
                })?;
                if index.is_some() && sub_index.is_some() {
                    return Err(format!(
                        "an inline parameter used as an assignment target is bound to an indexed name ('{}') and the statement indexes it too",
                        target
                    ));
                }
                *target = name;
                if index.is_none() {
                    *index = sub_index;
                }
            }
            **value = substitute_expr(value, subs);
            if let Some(i) = index {
                **i = substitute_expr(i, subs);
            }
        }
        Stmt::Send {
            channel,
            target,
            args,
            ..
        } => {
            **channel = substitute_expr(channel, subs);
            // `c ! v` parses with `v` as a bare-name target, which is what a parameter usually is. The
            // substitution has to replace it with the argument as an *expression*: an argument of `7`
            // left the target as the name `v`, which the runtime then read as a variable and rejected
            // with "chan_send: expected integer values".
            if let SendTarget::Ident(id) = target
                && let Some(sub) = subs.get(id.as_str())
            {
                *target = SendTarget::Value(sub.clone());
            }
            // The parser tries `expr` before `ident`, so a bare parameter name arrives as
            // Value(Ident("v")) rather than Ident("v"). Both spellings have to be substituted; the
            // first version only handled the second, and the send went out as s.P_v.
            match target {
                SendTarget::Value(v) | SendTarget::Sorted(v) => *v = substitute_expr(v, subs),
                SendTarget::Ident(_) => {}
            }
            for a in args.iter_mut() {
                *a = substitute_expr(a, subs);
            }
        }
        Stmt::Recv {
            channel, target, ..
        } => {
            **channel = substitute_expr(channel, subs);
            match target {
                RecvTarget::VarList(vars) | RecvTarget::Random(vars) => {
                    for v in vars.iter_mut() {
                        match subs.get(v.as_str()) {
                            Some(Expression::Ident(n)) => *v = n.clone(),
                            // A parameter bound to anything else cannot be a receive target: it would
                            // have to be assigned to, and an expression is not a place.
                            Some(_) => {
                                return Err(format!(
                                    "an inline parameter is used as a receive target and bound to something that is not a name: '{}'",
                                    v
                                ));
                            }
                            None => {}
                        }
                    }
                }
                RecvTarget::Eval(e) | RecvTarget::Poll(e) => {
                    *e = substitute_expr(e, subs);
                }
                RecvTarget::RandomMatch(es) => {
                    for e in es.iter_mut() {
                        *e = substitute_expr(e, subs);
                    }
                }
            }
        }
        Stmt::Assert(e, _) | Stmt::Expr(e, _) => *e = substitute_expr(e, subs),
        Stmt::Printf(_, args, _) => {
            for a in args.iter_mut() {
                *a = substitute_expr(a, subs);
            }
        }
        Stmt::Run(_, args, _) => {
            for a in args.iter_mut() {
                *a = substitute_expr(a, subs);
            }
        }
        Stmt::If(gs) | Stmt::Do(gs) => {
            for g in gs.iter_mut() {
                let Guard {
                    condition, body, ..
                } = g;
                if let Some(c) = condition {
                    *c = substitute_expr(c, subs);
                }
                *body = substitute_seq(body, subs)?;
            }
        }
        Stmt::Atomic(b, _) | Stmt::DStep(b, _) | Stmt::Block(b, _) => {
            *b = substitute_seq(b, subs)?;
        }
        Stmt::Unless { body, handler, .. } => {
            *body = substitute_seq(body, subs)?;
            *handler = substitute_seq(handler, subs)?;
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            **init = substitute_stmt(init, subs)?;
            *condition = substitute_expr(condition, subs);
            **update = substitute_stmt(update, subs)?;
            *body = substitute_seq(body, subs)?;
        }
        _ => {}
    }
    Ok(s)
}

pub(crate) fn substitute_expr(e: &Expression, subs: &BTreeMap<String, Expression>) -> Expression {
    match e {
        // The one rule: a parameter name is its argument, everywhere it appears.
        Expression::Ident(name) => match subs.get(name.as_str()) {
            Some(replacement) => replacement.clone(),
            None => e.clone(),
        },
        Expression::ArrayAccess { name, index } => Expression::ArrayAccess {
            name: name.clone(),
            index: Box::new(substitute_expr(index, subs)),
        },
        Expression::RecordAccess { record, field } => Expression::RecordAccess {
            record: Box::new(substitute_expr(record, subs)),
            field: field.clone(),
        },
        Expression::UnaryOp { op, expr } => Expression::UnaryOp {
            op: op.clone(),
            expr: Box::new(substitute_expr(expr, subs)),
        },
        Expression::BinaryOp { op, left, right } => Expression::BinaryOp {
            op: op.clone(),
            left: Box::new(substitute_expr(left, subs)),
            right: Box::new(substitute_expr(right, subs)),
        },
        Expression::FuncCall { name, args } => Expression::FuncCall {
            name: name.clone(),
            args: args.iter().map(|a| substitute_expr(a, subs)).collect(),
        },
        Expression::ChannelSend { channel, target } => Expression::ChannelSend {
            channel: channel.clone(),
            target: Box::new(substitute_expr(target, subs)),
        },
        Expression::ChannelPoll { channel, condition } => Expression::ChannelPoll {
            channel: channel.clone(),
            condition: Box::new(substitute_expr(condition, subs)),
        },
        Expression::PcValue(x) => Expression::PcValue(Box::new(substitute_expr(x, subs))),
        Expression::Eval(x) => Expression::Eval(Box::new(substitute_expr(x, subs))),
        Expression::GetPriority(x) => Expression::GetPriority(Box::new(substitute_expr(x, subs))),
        Expression::SetPriority { pid, value } => Expression::SetPriority {
            pid: Box::new(substitute_expr(pid, subs)),
            value: Box::new(substitute_expr(value, subs)),
        },
        Expression::RemoteRef { pid, name } => Expression::RemoteRef {
            pid: Box::new(substitute_expr(pid, subs)),
            name: name.clone(),
        },
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expanded(source: &str) -> String {
        let model = crate::parser::parse(source).expect("parse");
        let expanded = expand(&model).expect("expand");
        crate::codegen::generate(&expanded).source
    }

    #[test]
    fn an_argument_is_substituted_as_an_expression() {
        // `c ! v` parses with `v` as a bare-name target. Leaving it as a name made the runtime read a
        // variable that does not exist, so the send failed with "expected integer values".
        let lua = expanded(
            "inline snd(c, v) { c ! v }\nchan ch = [1] of { byte };\nactive proctype P() { snd(ch, 7) }\n",
        );
        assert!(
            lua.contains("'ch', 7)"),
            "the send inside the inline must carry the argument, got:\n{lua}"
        );
        assert!(
            !lua.contains("_v"),
            "no parameter name may survive, got:\n{lua}"
        );
    }

    #[test]
    fn an_argument_substitutes_into_an_assignment_target() {
        let lua =
            expanded("inline set(x) { x = 1 }\nbyte y = 0;\nactive proctype P() { set(y) }\n");
        assert!(lua.contains("s.y = "), "got:\n{lua}");
        assert!(
            !lua.contains("_x"),
            "no parameter name may survive, got:\n{lua}"
        );
    }

    #[test]
    fn a_nested_inline_is_expanded() {
        let lua = expanded(
            "inline inner(x) { x = x + 1 }\ninline outer(x) { inner(x); inner(x) }\nbyte y = 0;\nactive proctype P() { outer(y) }\n",
        );
        // The definition is listed in a header comment, so counting the call sites is what tells the
        // two apart: two nested calls give two assignment steps.
        assert_eq!(
            lua.matches("s.y = _spin_wrap").count(),
            2,
            "both nested calls must be expanded, got:\n{lua}"
        );
    }
}
