//! Code optimization passes for the Promela-to-Lua code generator.
//!
//! Provides four optimization passes:
//! 1. Dataflow analysis (GEN/KILL sets, IN/OUT sets)
//! 2. Dead variable elimination (remove unused vars from state vector)
//! 3. Statement merging (combine consecutive deterministic transitions)
//! 4. Rendezvous optimization (merge sync send/recv pairs)
//!
//! Each pass operates on the AST level and is controlled by a flag.

use std::collections::HashSet;

use crate::parser::ast::*;

/// Optimization level flags.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct OptLevel {
    /// Dataflow analysis (enables better optimization decisions)
    pub dataflow: bool,
    /// Dead variable elimination (-o2)
    pub dead_var_elim: bool,
    /// Statement merging (-o3)
    pub stmt_merging: bool,
    /// Rendezvous optimization (-o4)
    pub rendezvous: bool,
}

impl OptLevel {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn all() -> Self {
        Self {
            dataflow: true,
            dead_var_elim: true,
            stmt_merging: true,
            rendezvous: true,
        }
    }
}

// ─── Dataflow Analysis ─────────────────────────────────────────

/// A single transition in the control-flow graph.
#[derive(Debug, Clone)]
pub struct TransitionNode {
    /// Index in the transition list
    pub index: usize,
    /// Variables read by the guard (GEN set)
    pub gen_set: HashSet<String>,
    /// Variables written by the effect (KILL set)
    pub kill: HashSet<String>,
    /// Variables live on entry (IN set)
    pub in_set: HashSet<String>,
    /// Variables live on exit (OUT set)
    pub out_set: HashSet<String>,
    /// Predecessor indices in the CFG
    pub predecessors: Vec<usize>,
    /// Successor indices in the CFG
    pub successors: Vec<usize>,
    /// The statement this transition represents
    pub stmt: Stmt,
}

/// Dataflow analysis result for a proctype.
#[derive(Debug, Clone)]
pub struct DataflowResult {
    /// Transitions in order
    pub transitions: Vec<TransitionNode>,
    /// All variables used in this proctype
    pub all_vars: HashSet<String>,
    /// Dead variables (written but never subsequently read)
    pub dead_vars: HashSet<String>,
}

/// Extract variable names from an expression.
pub fn vars_in_expr(expr: &Expression) -> HashSet<String> {
    let mut vars = HashSet::new();
    collect_vars(expr, &mut vars);
    vars
}

fn collect_vars(expr: &Expression, vars: &mut HashSet<String>) {
    match expr {
        Expression::Ident(name) => {
            vars.insert(name.clone());
        }
        Expression::ArrayAccess { name, index } => {
            vars.insert(name.clone());
            collect_vars(index, vars);
        }
        Expression::RecordAccess { record, field: _ } => {
            collect_vars(record, vars);
        }
        Expression::UnaryOp { expr: e, .. } => {
            collect_vars(e, vars);
        }
        Expression::BinaryOp { left, right, .. } => {
            collect_vars(left, vars);
            collect_vars(right, vars);
        }
        Expression::FuncCall { name: _, args } => {
            for arg in args {
                collect_vars(arg, vars);
            }
        }
        Expression::ChannelSend { channel, target } => {
            vars.insert(channel.clone());
            collect_vars(target, vars);
        }
        Expression::ChannelPoll { channel, condition } => {
            vars.insert(channel.clone());
            collect_vars(condition, vars);
        }
        Expression::Len(name)
        | Expression::Full(name)
        | Expression::Empty(name)
        | Expression::NFull(name)
        | Expression::NEmpty(name)
        | Expression::Enabled(name) => {
            vars.insert(name.clone());
        }
        Expression::Timeout | Expression::NP_ => {}
        Expression::PcValue(expr) | Expression::Eval(expr) | Expression::GetPriority(expr) => {
            collect_vars(expr, vars);
        }
        Expression::SetPriority { pid, value } => {
            collect_vars(pid, vars);
            collect_vars(value, vars);
        }
        Expression::RemoteRef { pid, name } => {
            collect_vars(pid, vars);
            vars.insert(name.clone());
        }
        Expression::IntLit(_) | Expression::StringLit(_) | Expression::BoolLit(_) => {}
    }
}

/// Extract variables written by a statement's effect.
pub fn vars_written_by_stmt(stmt: &Stmt) -> HashSet<String> {
    let mut vars = HashSet::new();
    match stmt {
        Stmt::Assignment { target, .. } => {
            vars.insert(target.clone());
        }
        Stmt::Recv { target, .. } => {
            if let RecvTarget::VarList(vs) = target {
                for v in vs {
                    vars.insert(v.clone());
                }
            }
        }
        Stmt::Atomic(body, _) | Stmt::DStep(body, _) => {
            for s in body {
                vars.extend(vars_written_by_stmt(s));
            }
        }
        Stmt::If(guards) | Stmt::Do(guards) => {
            for g in guards {
                for s in &g.body {
                    vars.extend(vars_written_by_stmt(s));
                }
            }
        }
        Stmt::Unless { body, handler, .. } => {
            for s in body {
                vars.extend(vars_written_by_stmt(s));
            }
            for s in handler {
                vars.extend(vars_written_by_stmt(s));
            }
        }
        _ => {}
    }
    vars
}

/// Extract variables read by a statement's guard.
pub fn vars_read_by_stmt(stmt: &Stmt) -> HashSet<String> {
    let mut vars = HashSet::new();
    match stmt {
        Stmt::Assignment { value, .. } => {
            vars.extend(vars_in_expr(value));
        }
        Stmt::Assert(expr, _) => {
            vars.extend(vars_in_expr(expr));
        }
        Stmt::Send { channel, args, .. } => {
            vars.extend(vars_in_expr(channel));
            for a in args {
                vars.extend(vars_in_expr(a));
            }
        }
        Stmt::Recv { channel, .. } => {
            vars.extend(vars_in_expr(channel));
        }
        Stmt::If(guards) | Stmt::Do(guards) => {
            for g in guards {
                if let Some(cond) = &g.condition {
                    vars.extend(vars_in_expr(cond));
                }
                for s in &g.body {
                    vars.extend(vars_read_by_stmt(s));
                }
            }
        }
        Stmt::Atomic(body, _) | Stmt::DStep(body, _) => {
            for s in body {
                vars.extend(vars_read_by_stmt(s));
            }
        }
        Stmt::Unless { body, handler, .. } => {
            for s in body {
                vars.extend(vars_read_by_stmt(s));
            }
            for s in handler {
                vars.extend(vars_read_by_stmt(s));
            }
        }
        Stmt::Expr(e, _) => {
            vars.extend(vars_in_expr(e));
        }
        Stmt::Printf(_, args, _) => {
            for a in args {
                vars.extend(vars_in_expr(a));
            }
        }
        Stmt::Run(_, args, _) => {
            for a in args {
                vars.extend(vars_in_expr(a));
            }
        }
        Stmt::For { condition, update, body, .. } => {
            vars.extend(vars_in_expr(condition));
            vars.extend(vars_read_by_stmt(update));
            for s in body {
                vars.extend(vars_read_by_stmt(s));
            }
        }
        _ => {}
    }
    vars
}

/// Build a control-flow graph from a list of statements.
/// Returns a list of TransitionNodes with GEN/KILL sets computed.
pub fn build_cfg(stmts: &[Stmt]) -> Vec<TransitionNode> {
    let mut nodes = Vec::new();

    for (i, stmt) in stmts.iter().enumerate() {
        let gen_set = vars_read_by_stmt(stmt);
        let kill = vars_written_by_stmt(stmt);

        let mut predecessors = Vec::new();
        let mut successors = Vec::new();

        if i > 0 {
            predecessors.push(i - 1);
        }
        if i + 1 < stmts.len() {
            successors.push(i + 1);
        }

        nodes.push(TransitionNode {
            index: i,
            gen_set,
            kill,
            in_set: HashSet::new(),
            out_set: HashSet::new(),
            predecessors,
            successors,
            stmt: stmt.clone(),
        });
    }

    nodes
}

/// Compute IN/OUT sets via fixed-point iteration.
pub fn compute_in_out(nodes: &mut [TransitionNode]) {
    let mut changed = true;
    while changed {
        changed = false;
        for i in 0..nodes.len() {
            // IN[i] = union of OUT[p] for all predecessors p
            let mut new_in = HashSet::new();
            for &p in &nodes[i].predecessors {
                new_in.extend(nodes[p].out_set.clone());
            }

            // OUT[i] = (IN[i] - KILL[i]) union GEN[i]
            let mut new_out = new_in.clone();
            for k in &nodes[i].kill {
                new_out.remove(k);
            }
            for g in &nodes[i].gen_set {
                new_out.insert(g.clone());
            }

            if new_in != nodes[i].in_set || new_out != nodes[i].out_set {
                nodes[i].in_set = new_in;
                nodes[i].out_set = new_out;
                changed = true;
            }
        }
    }
}

/// Perform dataflow analysis on a proctype's body.
pub fn analyze_dataflow(stmts: &[Stmt]) -> DataflowResult {
    let mut transitions = build_cfg(stmts);
    compute_in_out(&mut transitions);

    // Collect all variables
    let mut all_vars = HashSet::new();
    for t in &transitions {
        all_vars.extend(t.gen_set.clone());
        all_vars.extend(t.kill.clone());
    }

    // Compute dead variables: written but never subsequently read
    let mut dead_vars = HashSet::new();
    for t in &transitions {
        for v in &t.kill {
            // A variable is dead if it's written but not in the OUT set
            // (meaning it's not read after this point before being written again)
            if !t.out_set.contains(v) {
                dead_vars.insert(v.clone());
            }
        }
    }

    DataflowResult {
        transitions,
        all_vars,
        dead_vars,
    }
}

// ─── Dead Variable Elimination ─────────────────────────────────

/// Remove dead variable declarations from a proctype's body.
/// Returns the set of variables that were removed.
pub fn eliminate_dead_vars(stmts: &mut [Stmt], dead_vars: &HashSet<String>) -> HashSet<String> {
    let mut removed = HashSet::new();

    for stmt in stmts.iter_mut() {
        match stmt {
            Stmt::Assignment { target, .. } => {
                let target = target.clone();
                if dead_vars.contains(&target) {
                    // Replace dead assignment with skip
                    *stmt = Stmt::Skip(0);
                    removed.insert(target);
                }
            }
            Stmt::Atomic(body, _) | Stmt::DStep(body, _) => {
                removed.extend(eliminate_dead_vars(body, dead_vars));
            }
            Stmt::If(guards) | Stmt::Do(guards) => {
                for g in guards.iter_mut() {
                    removed.extend(eliminate_dead_vars(&mut g.body, dead_vars));
                }
            }
            Stmt::Unless { body, handler, .. } => {
                removed.extend(eliminate_dead_vars(body, dead_vars));
                removed.extend(eliminate_dead_vars(handler, dead_vars));
            }
            _ => {}
        }
    }

    removed
}

/// Filter variable declarations to exclude dead variables.
pub fn filter_dead_var_decls<'a>(
    decls: Vec<&'a VarDecl>,
    dead_vars: &HashSet<String>,
    prefix: &str,
) -> Vec<&'a VarDecl> {
    decls
        .into_iter()
        .filter(|vd| {
            let full_name = format!("{}_{}", prefix, vd.name);
            !dead_vars.contains(&full_name)
        })
        .collect()
}

// ─── Statement Merging ─────────────────────────────────────────

/// Check if a statement is deterministic (no non-deterministic choice, no blocking).
pub fn is_deterministic(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Assignment { .. } => true,
        Stmt::Skip(_) => true,
        Stmt::Break(_) => true,
        Stmt::Goto(_, _) => true,
        Stmt::Expr(_, _) => true,
        Stmt::Printf(_, _, _) => true,
        Stmt::Assert(_, _) => true,
        Stmt::VarDecl(_) | Stmt::VarDecls(_) | Stmt::Label(_, _) => true,
        Stmt::Atomic(body, _) | Stmt::DStep(body, _) => body.iter().all(is_deterministic),
        Stmt::If(guards) => {
            // If with a single guard that has no condition (else) is deterministic
            guards.len() == 1 && guards[0].condition.is_none()
        }
        Stmt::Do(guards) => {
            // Do with a single guard that has no condition is deterministic
            guards.len() == 1 && guards[0].condition.is_none()
        }
        Stmt::For { .. } => true,
        Stmt::Unless { body, handler, .. } => {
            is_deterministic(body.first().unwrap_or(&Stmt::Skip(0)))
                && handler.iter().all(is_deterministic)
        }
        Stmt::Send { .. } | Stmt::Recv { .. } | Stmt::Run(_, _, _) => false,
    }
}

/// Check if a statement is a channel operation.
pub fn is_channel_op(stmt: &Stmt) -> bool {
    matches!(stmt, Stmt::Send { .. } | Stmt::Recv { .. })
}

/// Check if two consecutive statements are mergeable.
pub fn is_mergeable(a: &Stmt, b: &Stmt) -> bool {
    // Both must be deterministic
    if !is_deterministic(a) || !is_deterministic(b) {
        return false;
    }

    // Neither should be a channel operation
    if is_channel_op(a) || is_channel_op(b) {
        return false;
    }

    // A's effect shouldn't affect B's guard in a way that creates non-determinism
    // (conservative: always allow for now)
    true
}

/// Merge two consecutive statements into one.
/// The merged guard is the AND of both guards.
/// The merged effect is the sequence of both effects.
pub fn merge_stmts(a: &Stmt, b: &Stmt) -> Stmt {
    // For simple assignments, create a combined atomic block
    let mut body = Vec::new();
    body.push(a.clone());
    body.push(b.clone());
    Stmt::Atomic(body, 0)
}

/// Perform statement merging on a list of statements.
pub fn merge_consecutive(stmts: &[Stmt]) -> Vec<Stmt> {
    if stmts.is_empty() {
        return Vec::new();
    }

    let mut result = Vec::new();
    let mut i = 0;

    while i < stmts.len() {
        if i + 1 < stmts.len() && is_mergeable(&stmts[i], &stmts[i + 1]) {
            // Try to merge as many consecutive statements as possible
            let mut merged = stmts[i].clone();
            let mut j = i + 1;
            while j < stmts.len() && is_mergeable(&merged, &stmts[j]) {
                merged = merge_stmts(&merged, &stmts[j]);
                j += 1;
            }
            result.push(merged);
            i = j;
        } else {
            result.push(stmts[i].clone());
            i += 1;
        }
    }

    result
}

// ─── Rendezvous Optimization ───────────────────────────────────

/// Check if a channel is a sync channel (capacity 0).
pub fn is_sync_channel(channel: &Expression, model: &PromelaModel) -> bool {
    match channel {
        Expression::Ident(name) => {
            for decl in &model.declarations {
                if let TopLevel::ChanDecl { name: cn, capacity, .. } = decl
                    && cn == name {
                        return *capacity == 0;
                    }
            }
            false
        }
        _ => false,
    }
}

/// Check if a statement is a send on a sync channel.
pub fn is_sync_send(stmt: &Stmt, model: &PromelaModel) -> bool {
    match stmt {
        Stmt::Send { channel, .. } => is_sync_channel(channel, model),
        _ => false,
    }
}

/// Check if a statement is a receive on a specific channel.
pub fn is_recv_on_channel(stmt: &Stmt, channel: &Expression) -> bool {
    match stmt {
        Stmt::Recv { channel: c, .. } => {
            match (channel, c.as_ref()) {
                (Expression::Ident(n1), Expression::Ident(n2)) => n1 == n2,
                _ => false,
            }
        }
        _ => false,
    }
}

/// Perform rendezvous optimization: merge sync send/recv pairs.
pub fn optimize_rendezvous(stmts: &[Stmt], model: &PromelaModel) -> Vec<Stmt> {
    if stmts.is_empty() {
        return Vec::new();
    }

    let mut result = Vec::new();
    let mut i = 0;

    while i < stmts.len() {
        if i + 1 < stmts.len()
            && let Stmt::Send { channel, .. } = &stmts[i]
                && is_sync_channel(channel, model) && is_recv_on_channel(&stmts[i + 1], channel) {
                    // Merge sync send/recv pair into atomic block
                    let mut body = Vec::new();
                    body.push(stmts[i].clone());
                    body.push(stmts[i + 1].clone());
                    result.push(Stmt::Atomic(body, 0));
                    i += 2;
                    continue;
                }
        result.push(stmts[i].clone());
        i += 1;
    }

    result
}

// ─── Top-Level Optimization Pipeline ───────────────────────────

/// Apply all enabled optimizations to a proctype's body.
pub fn optimize_proctype(
    stmts: &[Stmt],
    model: &PromelaModel,
    opt: &OptLevel,
) -> Vec<Stmt> {
    let mut result = stmts.to_vec();

    if opt.dataflow || opt.dead_var_elim {
        let dataflow = analyze_dataflow(&result);

        if opt.dead_var_elim {
            eliminate_dead_vars(&mut result, &dataflow.dead_vars);
        }
    }

    if opt.stmt_merging {
        result = merge_consecutive(&result);
    }

    if opt.rendezvous {
        result = optimize_rendezvous(&result, model);
    }

    result
}

/// Apply optimizations to an entire model, returning a new optimized model.
pub fn apply_to_model(model: &PromelaModel, opt: &OptLevel) -> PromelaModel {
    let mut declarations = Vec::new();

    for decl in &model.declarations {
        match decl {
            TopLevel::Proctype(p) => {
                let optimized_body = optimize_proctype(&p.body, model, opt);
                declarations.push(TopLevel::Proctype(ProctypeDef {
                    body: optimized_body,
                    ..p.clone()
                }));
            }
            TopLevel::Init(i) => {
                let optimized_body = optimize_proctype(&i.body, model, opt);
                declarations.push(TopLevel::Init(InitDef {
                    body: optimized_body,
                    ..i.clone()
                }));
            }
            TopLevel::NeverClaim(n) => {
                let optimized_body = optimize_proctype(&n.body, model, opt);
                declarations.push(TopLevel::NeverClaim(NeverClaim {
                    body: optimized_body,
                    ..n.clone()
                }));
            }
            _ => {
                declarations.push(decl.clone());
            }
        }
    }

    PromelaModel {
        declarations,
        source: model.source.clone(),
        mtype_names: model.mtype_names.clone(),
        typedefs: model.typedefs.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_assign(target: &str, value: &str) -> Stmt {
        Stmt::Assignment {
            target: target.to_string(),
            index: None,
            field: None,
            value: Box::new(Expression::Ident(value.to_string())),
            line: 0,
        }
    }

    fn make_skip() -> Stmt {
        Stmt::Skip(0)
    }

    fn make_send(ch: &str) -> Stmt {
        Stmt::Send {
            channel: Box::new(Expression::Ident(ch.to_string())),
            target: SendTarget::Value(Expression::IntLit(1)),
            args: vec![],
            line: 0,
        }
    }

    fn make_recv(ch: &str, var: &str) -> Stmt {
        Stmt::Recv {
            channel: Box::new(Expression::Ident(ch.to_string())),
            target: RecvTarget::VarList(vec![var.to_string()]),
            line: 0,
        }
    }

    #[test]
    fn test_vars_in_expr_ident() {
        let expr = Expression::Ident("x".to_string());
        let vars = vars_in_expr(&expr);
        assert!(vars.contains("x"));
        assert_eq!(vars.len(), 1);
    }

    #[test]
    fn test_vars_in_expr_binary() {
        let expr = Expression::BinaryOp {
            left: Box::new(Expression::Ident("x".to_string())),
            op: BinaryOp::Add,
            right: Box::new(Expression::Ident("y".to_string())),
        };
        let vars = vars_in_expr(&expr);
        assert!(vars.contains("x"));
        assert!(vars.contains("y"));
        assert_eq!(vars.len(), 2);
    }

    #[test]
    fn test_gen_kill_simple() {
        let stmt = Stmt::Assignment {
            target: "x".to_string(),
            index: None,
            field: None,
            value: Box::new(Expression::BinaryOp {
                left: Box::new(Expression::Ident("y".to_string())),
                op: BinaryOp::Add,
                right: Box::new(Expression::IntLit(1)),
            }),
            line: 0,
        };

        let gen_set = vars_read_by_stmt(&stmt);
        let kill = vars_written_by_stmt(&stmt);

        assert!(gen_set.contains("y"));
        assert!(!gen_set.contains("x"));
        assert!(kill.contains("x"));
    }

    #[test]
    fn test_dataflow_simple() {
        let stmts = vec![
            make_assign("x", "1"),
            Stmt::Assignment {
                target: "y".to_string(),
                index: None,
                field: None,
                value: Box::new(Expression::BinaryOp {
                    left: Box::new(Expression::Ident("x".to_string())),
                    op: BinaryOp::Add,
                    right: Box::new(Expression::IntLit(1)),
                }),
                line: 0,
            },
        ];

        let result = analyze_dataflow(&stmts);
        assert_eq!(result.transitions.len(), 2);
        // First transition: x = 1 reads "1" as ident, so GEN contains "1"
        assert!(result.transitions[0].kill.contains("x"));
        // Second transition: y = x + 1 reads x, GEN contains x
        assert!(result.transitions[1].gen_set.contains("x"));
        assert!(result.transitions[1].kill.contains("y"));
    }

    #[test]
    fn test_dataflow_loop() {
        let stmts = vec![
            Stmt::Do(vec![
                Guard {
                    condition: Some(Expression::BoolLit(true)),
                    body: vec![make_assign("x", "x")],
                    line: 0,
                },
                Guard {
                    condition: Some(Expression::BoolLit(true)),
                    body: vec![make_assign("y", "y")],
                    line: 0,
                },
            ]),
        ];

        let result = analyze_dataflow(&stmts);
        assert!(!result.transitions.is_empty());
    }

    #[test]
    fn test_is_deterministic() {
        assert!(is_deterministic(&make_assign("x", "1")));
        assert!(is_deterministic(&make_skip()));
        assert!(!is_deterministic(&make_send("ch")));
    }

    #[test]
    fn test_merge_consecutive_assignments() {
        let stmts = vec![
            make_assign("x", "1"),
            make_assign("y", "2"),
            make_assign("z", "x"),
        ];

        let merged = merge_consecutive(&stmts);
        assert!(merged.len() < stmts.len());
    }

    #[test]
    fn test_non_mergeable_not_merged() {
        let stmts = vec![
            make_assign("x", "1"),
            make_send("ch"),
            make_assign("y", "2"),
        ];

        let merged = merge_consecutive(&stmts);
        assert_eq!(merged.len(), 3);
    }

    #[test]
    fn test_dead_var_elimination() {
        let mut stmts = vec![
            make_assign("x", "1"),
            make_assign("y", "2"),
            make_assign("z", "x"),
        ];

        let dataflow = analyze_dataflow(&stmts);
        eliminate_dead_vars(&mut stmts, &dataflow.dead_vars);
    }

    #[test]
    fn test_rendezvous_optimization() {
        let model = PromelaModel {
            declarations: vec![TopLevel::ChanDecl {
                name: "ch".to_string(),
                capacity: 0,
                line: 0,
            }],
            source: None,
            mtype_names: HashMap::new(),
            typedefs: HashMap::new(),
        };

        let stmts = vec![
            make_send("ch"),
            make_recv("ch", "x"),
        ];

        let optimized = optimize_rendezvous(&stmts, &model);
        assert_eq!(optimized.len(), 1);
        assert!(matches!(optimized[0], Stmt::Atomic(_, _)));
    }

    #[test]
    fn test_async_channel_not_affected() {
        let model = PromelaModel {
            declarations: vec![TopLevel::ChanDecl {
                name: "ch".to_string(),
                capacity: 1,
                line: 0,
            }],
            source: None,
            mtype_names: HashMap::new(),
            typedefs: HashMap::new(),
        };

        let stmts = vec![
            make_send("ch"),
            make_recv("ch", "x"),
        ];

        let optimized = optimize_rendezvous(&stmts, &model);
        assert_eq!(optimized.len(), 2);
    }
}
