//! Lua code generator: compiles Promela IR into Lua scripts.
//!
//! The generated Lua is loaded by the mlua runtime and called by the
//! verification engine for model-specific transition enumeration.

use crate::parser::ast::*;

/// Generated Lua code components.
#[derive(Debug, Clone)]
pub struct GeneratedLua {
    /// The full Lua source as a string.
    pub source: String,
    /// Per-proctype transition enumerator names.
    pub proctype_fn_names: Vec<String>,
}

/// Compile a Promela model into Lua source code.
pub fn generate(model: &PromelaModel) -> GeneratedLua {
    let mut g = LuaGenerator::new();
    g.emit_header();
    g.emit_state_layout(model);
    g.emit_proctypes(model);
    g.emit_trailer();
    g.finish()
}

/// Translate one Promela expression into Lua against the model's globals.
///
/// A claim's atom is evaluated against a state rather than generated as part of a transition, so
/// it needs the same name mapping the emitters use - global variables are stored under their own
/// names - but no proctype context.
pub fn expression_to_lua(model: &PromelaModel, e: &Expression) -> String {
    let mut g = LuaGenerator::new();
    for decl in &model.declarations {
        match decl {
            TopLevel::GlobalVar(v) => {
                g.global_vars.insert(v.name.clone());
            }
            TopLevel::ChanDecl { name, .. } => {
                g.global_vars.insert(name.clone());
            }
            _ => {}
        }
    }
    g.current_proctype = None;
    g.expr_to_lua(e)
}

/// Compile a Promela model into Lua source code with optimizations.
pub fn generate_with_opt(model: &PromelaModel, opt: &optimize::OptLevel) -> GeneratedLua {
    let optimized = optimize::apply_to_model(model, opt);
    let mut g = LuaGenerator::new();
    g.emit_header();
    g.emit_state_layout(&optimized);
    g.emit_proctypes(&optimized);
    g.emit_trailer();
    g.finish()
}

/// The aliases every generated chunk may use, and the prelude of an atom evaluation.
///
/// Kept in one place because they were in two: an atom is compiled as its own chunk against a bare
/// state, so `len(q) < 2` in a claim called `chan_len` and found nil, while the same expression in
/// a body resolved through the generated model's aliases. Two definitions of the same names is how
/// that happens.
pub(crate) const ALIASES: &str = "local chan_send = _spin_chan_send; \
     local chan_recv = _spin_chan_recv; local chan_len = _spin_chan_len; \
     local chan_full = _spin_chan_full; local chan_empty = _spin_chan_empty; \
     local assert = _spin_assert; local printf = _spin_printf; \
     local state_hash = _spin_state_hash; ";

pub(crate) struct LuaGenerator {
    pub(crate) source: String,
    pub(crate) indent: usize,
    pub(crate) proctype_names: Vec<String>,
    pub(crate) current_proctype: Option<String>,
    /// The `provided (expr)` condition of the proctype being emitted, if it has one.
    pub(crate) current_provided: Option<Expression>,
    /// The field that records this body's process as finished.
    ///
    /// A proctype uses `_done_<name>` and an `init` block uses `_done_init`, which the state
    /// declares and the init transition function tests. The flattened body wrote `_done` when
    /// there was no proctype name, so an init block never marked itself finished and every model
    /// with an init block ended in a reported deadlock.
    pub(crate) current_done: Option<String>,
    /// The state field that holds this body's program counter.
    ///
    /// It was derived from the proctype name, so an `init` block emitted guards on `s._step`, a
    /// field no state declares: every guard was false, the init never ran, and the model was
    /// reported as a deadlock.
    pub(crate) current_step: Option<String>,
    /// Channels declared with capacity 0, which pan runs as a single transition over both
    /// processes, and the proctypes that send on each of them.
    ///
    /// A rendezvous send is staged by the sender and released by the receiver, so the receiver's
    /// generated code has to write the stager's program counter. That is only possible if it knows
    /// which proctypes can be on the other side, hence this map, filled from the AST before any
    /// body is emitted.
    pub(crate) rendezvous: std::collections::BTreeMap<String, Vec<(String, usize)>>,
    /// Fields per message, per channel name, from the declaration. pan refuses a send or a receive
    /// whose value count differs from it at run time - there is no verdict to match - so the
    /// generator refuses that shape by name instead of binding what it can and dropping the rest.
    pub(crate) channel_fields: std::collections::BTreeMap<String, usize>,
    /// Ranges of a body's steps that an `unless` handler preempts, with the handler's guard.
    ///
    /// A body step inside one of these is disabled exactly where the handler is executable: pan's
    /// `P unless Q` gives Q priority, so the body may not take a step once the escape can fire.
    /// Cleared per body.
    pub(crate) preempt: Vec<(usize, usize, String)>,
    /// Declared type of every state field, keyed by the field name the state stores.
    ///
    /// Promela's types are fixed width and assignments wrap, so the value a variable holds is not
    /// the value the expression produced.
    pub(crate) var_types: std::collections::HashMap<String, VarType>,
    pub(crate) global_vars: std::collections::HashSet<String>,
    /// The name an emitted expression indexes for state fields.
    ///
    /// Transitions are wrapped in a function of `s`, so it is `s` there. The state layout is not: it
    /// builds a local table called `state`, and emitting `s.` inside it produced `state.P_x = s.a` -
    /// an index of a nil global, which failed the whole initial state and, before that failure was
    /// reported, made the model explore nothing and pass.
    pub(crate) state_var: String,
    pub(crate) inlines: std::collections::HashMap<String, crate::parser::ast::InlineDef>,
    /// Label step numbers for goto/break/label: label_name -> step_number
    pub(crate) label_steps: std::collections::HashMap<String, usize>,
    /// Do-loop exit steps: stack of exit step numbers for nested do-loops
    pub(crate) do_exit_stack: Vec<usize>,
    /// Next available step number for labels (starts after guard body steps)
    pub(crate) next_label_step: usize,
}

mod core;
mod deps;
mod effects;
mod effects_guards;
mod expr_utils;
mod flatten;
mod flatten_rdv;
mod flatten_refuse;
mod flatten_unless;
pub(crate) mod inline;
pub mod optimize;
mod stmts;
mod stmts_atomic;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod expr_tests;

/// Walk every statement of a body, children included, calling `f` on each.
///
/// Used to answer model-level questions - does any statement use `timeout`, does any call an inline -
/// before anything is generated. A per-statement hook would answer them at emission time and could be
/// bypassed by a statement that reaches an arm the hook does not know about; this cannot.
pub(crate) fn walk_stmts<'a>(
    body: &'a [crate::parser::ast::Stmt],
    f: &mut dyn FnMut(&'a crate::parser::ast::Stmt),
) {
    use crate::parser::ast::Stmt;
    for s in body {
        f(s);
        match s {
            Stmt::If(gs) | Stmt::Do(gs) => {
                for g in gs {
                    walk_stmts(&g.body, f);
                }
            }
            Stmt::Atomic(b, _) | Stmt::DStep(b, _) | Stmt::Block(b, _) => walk_stmts(b, f),
            Stmt::Unless { body, handler, .. } => {
                walk_stmts(body, f);
                walk_stmts(handler, f);
            }
            Stmt::For {
                init, update, body, ..
            } => {
                walk_stmts(std::slice::from_ref(init.as_ref()), f);
                walk_stmts(std::slice::from_ref(update.as_ref()), f);
                walk_stmts(body, f);
            }
            _ => {}
        }
    }
}
/// Walk an expression, children included.
pub(crate) fn walk_expr<'a>(
    e: &'a crate::parser::ast::Expression,
    f: &mut dyn FnMut(&'a crate::parser::ast::Expression),
) {
    use crate::parser::ast::Expression as E;
    f(e);
    match e {
        E::ArrayAccess { index, .. } => walk_expr(index, f),
        E::RecordAccess { record, .. } => walk_expr(record, f),
        E::UnaryOp { expr, .. } => walk_expr(expr, f),
        E::BinaryOp { left, right, .. } => {
            walk_expr(left, f);
            walk_expr(right, f);
        }
        E::FuncCall { args, .. } => {
            for a in args {
                walk_expr(a, f);
            }
        }
        E::ChannelSend { target, .. } => walk_expr(target, f),
        E::ChannelPoll { condition, .. } => walk_expr(condition, f),
        E::PcValue(x) | E::Eval(x) | E::GetPriority(x) => walk_expr(x, f),
        E::SetPriority { pid, value } => {
            walk_expr(pid, f);
            walk_expr(value, f);
        }
        E::RemoteRef { pid, .. } => walk_expr(pid, f),
        _ => {}
    }
}

/// Every expression a statement mentions, in the order they appear.
fn stmt_exprs<'a>(
    s: &'a crate::parser::ast::Stmt,
    out: &mut Vec<&'a crate::parser::ast::Expression>,
) {
    use crate::parser::ast::{RecvTarget, SendTarget, Stmt};
    let mut push = |e: &'a crate::parser::ast::Expression| out.push(e);
    match s {
        Stmt::Assignment { value, index, .. } => {
            push(value);
            if let Some(i) = index {
                push(i);
            }
        }
        Stmt::Assert(e, _) | Stmt::Expr(e, _) => push(e),
        Stmt::Printf(_, args, _) => args.iter().for_each(push),
        Stmt::Send {
            channel,
            target,
            args,
            ..
        } => {
            push(channel);
            match target {
                SendTarget::Value(v) | SendTarget::Sorted(v) => push(v),
                SendTarget::Ident(_) => {}
            }
            args.iter().for_each(push);
        }
        Stmt::Recv {
            channel, target, ..
        } => {
            push(channel);
            match target {
                RecvTarget::Eval(e) | RecvTarget::Poll(e) => push(e),
                RecvTarget::RandomMatch(es) => es.iter().for_each(push),
                RecvTarget::VarList(_) | RecvTarget::Random(_) => {}
            }
        }
        Stmt::Run(_, args, _) => args.iter().for_each(push),
        Stmt::For { condition, .. } => push(condition),
        // A guard is where `timeout` is normally written. Walking bodies alone missed it, which is
        // how the first version of this check let the shape through.
        Stmt::If(gs) | Stmt::Do(gs) => {
            for g in gs {
                if let Some(c) = &g.condition {
                    push(c);
                }
            }
        }
        _ => {}
    }
}

/// Does any statement of this body reach a `timeout`, or call a named inline?
///
/// Both are answered before anything is generated: `_spin_timeout` returns false unconditionally
/// today, so a `timeout` branch is dead and a model that needed it was refuted where pan accepts
/// it; and an inline call emitted by the step machine became a call to a Lua global that nothing
/// defines.
pub(crate) fn model_shape_refusal(model: &crate::parser::ast::PromelaModel) -> Option<String> {
    use crate::parser::ast::{Expression as E, TopLevel};
    // `provided` gates a process, and this implementation of that gate disagrees with pan on one
    // model in spin's own Examples: pathfinder.pml refutes an invalid end state there (pan 1) and
    // this engine explores 6 states and reports none, where dropping the clause gives 25 states and
    // agreement on 0. Minimal `provided` models do agree, so the disagreement is in the interaction
    // with a blocking atomic region and is not understood. A clause that hides a deadlock is a wrong
    // verdict, so it refuses by name until it is.
    if let Some(p) = model.declarations.iter().find_map(|d| match d {
        TopLevel::Proctype(p) if p.provided.is_some() => Some(p),
        _ => None,
    }) {
        return Some(format!(
            "provided is not implemented: the clause gates proctype '{}', and this engine''s gate hides a deadlock pan reports (pathfinder.pml)",
            p.name
        ));
    }
    for (name, body) in model.declarations.iter().filter_map(|d| match d {
        TopLevel::Proctype(p) => Some((p.name.clone(), &p.body)),
        TopLevel::Init(i) => Some(("init".to_string(), &i.body)),
        _ => None,
    }) {
        let p = &name;
        let mut found: Option<String> = None;
        walk_stmts(body, &mut |s| {
            if found.is_some() {
                return;
            }
            // A local's initialiser is evaluated once, into the state layout, where `_pid` is not
            // bound - it is the id of the process whose transition this is. The initial state then
            // failed to build, which used to be swallowed into an empty state space and a clean
            // verdict. Refused by name until initialisers are evaluated per instance.
            let decls: Vec<&crate::parser::ast::VarDecl> = match s {
                crate::parser::ast::Stmt::VarDecl(d) => vec![d],
                crate::parser::ast::Stmt::VarDecls(ds) => ds.iter().collect(),
                _ => Vec::new(),
            };
            for d in decls {
                let Some(init) = &d.init else { continue };
                let mut uses_pid = false;
                walk_expr(init, &mut |x| {
                    if matches!(x, E::Ident(n) if n == "_pid") {
                        uses_pid = true;
                    }
                });
                if uses_pid {
                    found = Some(format!(
                        "a local variable initialised from _pid is not supported: the initialiser runs once for the whole state, and _pid is bound per instance; in '{}'",
                        p
                    ));
                    return;
                }
            }
            let mut es = Vec::new();
            stmt_exprs(s, &mut es);
            // A call the emitter does not know becomes a Lua call to a global nothing defines, which
            // fails at run time with "attempt to call a nil value". After inline expansion there is no
            // such thing as a user-defined function left, so any call still here is unknown.
            for e in &es {
                let mut unknown: Option<String> = None;
                walk_expr(e, &mut |x| {
                    match x {
                        E::FuncCall { name, .. } if unknown.is_none() => {
                            unknown = Some(name.clone());
                        }
                        // pan's set_priority changes the scheduling order at run time; the priority
                        // filter here reads the priorities declared on the proctype, so a model that
                        // moves them would be scheduled by pan and not by us.
                        E::SetPriority { .. } if unknown.is_none() => {
                            unknown = Some("set_priority".to_string());
                        }
                        _ => {}
                    }
                });
                if let Some(name) = unknown {
                    found = Some(format!(
                        "a call to '{}' is not implemented: a function this engine does not define would fail at run time, and set_priority also changes the scheduling order at run time; in '{}'",
                        name, p
                    ));
                    return;
                }
            }
            for e in es {
                let mut timed_out = false;
                walk_expr(e, &mut |x| {
                    if matches!(x, E::Timeout) {
                        timed_out = true;
                    }
                });
                if timed_out {
                    found = Some(format!(
                        "timeout is not implemented: it is true only when no other process can move, and a guard here cannot ask that; in '{}'",
                        p
                    ));
                    return;
                }
            }
        });
        if found.is_some() {
            return found;
        }
    }
    None
}
/// Does this body contain an `unless`? Only then does the second counter have a reader.
pub(crate) fn stmts_have_unless(body: &[crate::parser::ast::Stmt]) -> bool {
    use crate::parser::ast::Stmt;
    body.iter().any(|s| match s {
        Stmt::Unless { .. } => true,
        Stmt::If(gs) | Stmt::Do(gs) => gs.iter().any(|g| stmts_have_unless(&g.body)),
        Stmt::Atomic(b, _) | Stmt::DStep(b, _) | Stmt::Block(b, _) => stmts_have_unless(b),
        Stmt::For { body, .. } => stmts_have_unless(body),
        _ => false,
    })
}

/// The rendezvous channels a body sends on, found without laying the body out.
fn sends_on(
    body: &[crate::parser::ast::Stmt],
    zero_capacity: &std::collections::HashSet<String>,
) -> Vec<String> {
    use crate::parser::ast::{Expression, Stmt};
    fn channel_name(e: &Expression) -> Option<String> {
        match e {
            Expression::Ident(n) => Some(n.clone()),
            Expression::ArrayAccess { name, .. } => Some(name.clone()),
            _ => None,
        }
    }
    fn walk(stmts: &[Stmt], out: &mut Vec<String>, zero: &std::collections::HashSet<String>) {
        for s in stmts {
            match s {
                Stmt::Send { channel, .. } => {
                    if let Some(n) = channel_name(channel)
                        && zero.contains(&n)
                        && !out.contains(&n)
                    {
                        out.push(n);
                    }
                }
                Stmt::If(gs) | Stmt::Do(gs) => {
                    for g in gs {
                        walk(&g.body, out, zero);
                    }
                }
                Stmt::Atomic(b, _) | Stmt::DStep(b, _) => walk(b, out, zero),
                Stmt::Unless { body, handler, .. } => {
                    walk(body, out, zero);
                    walk(handler, out, zero);
                }
                Stmt::For { body, .. } => walk(body, out, zero),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(body, &mut out, zero_capacity);
    out
}

pub(crate) fn default_value(var_type: &VarType) -> String {
    match var_type {
        VarType::Bit | VarType::Bool | VarType::Byte => "0".to_string(),
        VarType::Short | VarType::Int | VarType::Unsigned(_) => "0".to_string(),
        VarType::Chan => "nil".to_string(),
        VarType::Mtype => "0".to_string(),
        VarType::Named(_) => "nil".to_string(),
    }
}
