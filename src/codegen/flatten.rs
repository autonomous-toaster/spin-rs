//! Control flow compiled to a step space.
//!
//! A proctype body is laid out as a graph: one step per statement, one selection step per
//! `if`/`do`, and every jump resolved to a step number at generation time. The generator used to
//! carry five counters - one per construct - and only the construct that owned one could express
//! its own sequencing, so a `do` inside a sequence could not be expressed at all.

use super::*;

/// One step of a flattened proctype body.
///
/// Control flow is resolved at generation time, so a nested selection is not a counter of its own
/// but a step whose options jump elsewhere in the same step space. That is what lets a `do` sit
/// inside a sequence, which the previous step machine could not express and refused.
enum FNode {
    /// A statement that runs at one step and continues at `next`.
    Simple { stmt: Stmt, next: usize },
    /// A selection: one transition per option, each jumping to the branch it chose.
    Select {
        options: Vec<(Option<Expression>, usize)>,
    },
    /// An unconditional jump.
    Jump(usize),
    /// A `goto` whose target is resolved once every label has a step number.
    Goto(String),
    /// A region run as one transition: pan does not make the states between an atomic
    /// sequence's statements observable, so neither do we.
    Fused {
        guard: String,
        effect: String,
        next: usize,
    },
    /// The process ends here.
    Halt,
    /// A construct with no encoding yet. Refused rather than reordered or dropped.
    Refuse(&'static str),
}

impl LuaGenerator {
    /// The number of steps a statement occupies once flattened.
    fn fsize(&self, s: &Stmt) -> usize {
        match s {
            Stmt::If(gs) | Stmt::Do(gs) => {
                let bodies: usize = gs
                    .iter()
                    .map(|g| g.body.iter().map(|b| self.fsize(b)).sum::<usize>())
                    .sum();
                1 + bodies
            }
            _ => 1,
        }
    }

    /// Lay out a statement sequence starting at `base`. Returns the next free step.
    ///
    /// `cont` is where the last statement falls through to, `brk` is where `break` goes.
    fn flayout_seq(
        &mut self,
        stmts: &[Stmt],
        base: usize,
        cont: usize,
        brk: usize,
        nodes: &mut Vec<FNode>,
        labels: &mut Vec<(String, usize)>,
    ) -> usize {
        let mut starts = Vec::with_capacity(stmts.len());
        let mut idx = base;
        for s in stmts {
            starts.push(idx);
            idx += self.fsize(s);
        }
        for (i, s) in stmts.iter().enumerate() {
            let next = if i + 1 < stmts.len() {
                starts[i + 1]
            } else {
                cont
            };
            self.flayout_stmt(s, starts[i], next, brk, nodes, labels);
        }
        idx
    }

    fn flayout_stmt(
        &mut self,
        s: &Stmt,
        at: usize,
        next: usize,
        brk: usize,
        nodes: &mut Vec<FNode>,
        labels: &mut Vec<(String, usize)>,
    ) {
        match s {
            // A branch continues at the join; a loop body continues at its own selection, and a
            // break leaves it for `next`.
            Stmt::If(gs) | Stmt::Do(gs) => {
                let is_loop = matches!(s, Stmt::Do(_));
                let mut cursor = at + 1;
                let mut options = Vec::new();
                for g in gs {
                    let start = cursor;
                    let (body_cont, body_brk) = if is_loop { (at, next) } else { (next, brk) };
                    cursor = self.flayout_seq(&g.body, start, body_cont, body_brk, nodes, labels);
                    options.push((g.condition.clone(), start));
                }
                nodes[at] = FNode::Select { options };
            }
            Stmt::Break(_) => nodes[at] = FNode::Jump(brk),
            Stmt::Goto(l, _) => nodes[at] = FNode::Goto(l.clone()),
            Stmt::Label(name, _) => {
                labels.push((name.clone(), at));
                nodes[at] = FNode::Jump(next);
            }
            // pan runs an atomic sequence as one transition, so the states between its
            // statements are not observable. Fuse them into one step: the guards are conjoined and
            // the effects concatenated. Anything that needs its own control flow cannot be fused
            // and is refused rather than emitted as a region that is not atomic.
            Stmt::Atomic(b, _) | Stmt::DStep(b, _) => {
                if b.is_empty() {
                    nodes[at] = FNode::Jump(next);
                } else if b.iter().all(is_simple_stmt) {
                    let mut guard = String::new();
                    let mut effect = String::new();
                    for st in b {
                        let (g, e) = self.fstmt_parts(st);
                        guard.push_str(&g);
                        effect.push_str(&e);
                    }
                    nodes[at] = FNode::Fused {
                        guard,
                        effect,
                        next,
                    };
                } else {
                    nodes[at] = FNode::Refuse(
                        "an atomic/d_step region containing control flow is not fused",
                    );
                }
            }
            Stmt::For { .. } => nodes[at] = FNode::Refuse("for"),
            Stmt::Run(_, _, _) => nodes[at] = FNode::Refuse("run"),
            Stmt::Unless { .. } => nodes[at] = FNode::Refuse("unless"),
            _ => {
                nodes[at] = FNode::Simple {
                    stmt: s.clone(),
                    next,
                }
            }
        }
    }

    /// Declare a transition that only moves the program counter.
    ///
    /// These used to be emitted opaque, which is sound but tells the reduction that the step
    /// cannot be independent of anything: POR stopped reducing entirely (16807 states where it
    /// had stored 3125). A step writes only its own bookkeeping, so it is internal and invisible.
    fn emit_step_deps(&mut self, step_var: &str, done: Option<String>) {
        let mut deps = self.deps_of(None, &[], &[step_var]);
        if let Some(d) = done {
            deps.write_field(d);
        }
        self.emit_deps(&deps);
    }

    /// The extra guard and the leading effect of a simple statement.
    fn fstmt_parts(&mut self, stmt: &Stmt) -> (String, String) {
        match stmt {
            Stmt::Assignment {
                target,
                index,
                value,
                ..
            } => {
                let v = self.expr_to_lua(value);
                let target_name = if self.global_vars.contains(target.as_str()) {
                    target.to_string()
                } else if let Some(ref pname) = self.current_proctype {
                    format!("{}_{}", pname, target)
                } else {
                    target.to_string()
                };
                let target_expr = if let Some(idx) = index {
                    format!("s.{}[{} + 1]", target_name, self.expr_to_lua(idx))
                } else {
                    format!("s.{}", target_name)
                };
                (String::new(), format!("{} = {}; ", target_expr, v))
            }
            // A blocking expression is a statement in its own right: enabled only while it holds.
            Stmt::Expr(e, _) => (format!(" and ({})", self.expr_to_lua(e)), String::new()),
            Stmt::Assert(e, _) => {
                let cond = self.expr_to_lua(e);
                let check = self.assert_failure_effect(&cond, &format!("assert({})", cond));
                (String::new(), format!("{}; ", check))
            }
            Stmt::Send {
                channel,
                target,
                args,
                ..
            } => {
                let chan = self.channel_to_lua(channel);
                let mut values: Vec<String> = vec![match target {
                    SendTarget::Ident(id) => id.clone(),
                    SendTarget::Value(v) => self.expr_to_lua(v),
                    SendTarget::Sorted(v) => self.expr_to_lua(v),
                }];
                values.extend(args.iter().map(|a| self.expr_to_lua(a)));
                (
                    format!(" and not chan_full({})", chan),
                    format!("_spin_chan_send(s, {}, {}); ", chan, values.join(", ")),
                )
            }
            Stmt::Recv {
                channel, target, ..
            } => {
                let chan = self.channel_to_lua(channel);
                let vars = match target {
                    RecvTarget::VarList(v) => v
                        .iter()
                        .map(|v| {
                            let name = if self.global_vars.contains(v.as_str()) {
                                v.clone()
                            } else if let Some(ref p) = self.current_proctype {
                                format!("{}_{}", p, v)
                            } else {
                                v.clone()
                            };
                            format!("s.{}", name)
                        })
                        .collect::<Vec<_>>()
                        .join(", "),
                    _ => return (String::new(), String::new()),
                };
                (
                    format!(" and not chan_empty({})", chan),
                    format!("_spin_chan_recv(s, {}, {}); ", chan, vars),
                )
            }
            _ => (String::new(), String::new()),
        }
    }

    /// Emit a proctype body as one transition per step of a flattened control-flow graph.
    pub(crate) fn emit_flattened_body(&mut self, body: &[Stmt], _depth: usize) {
        let exec: Vec<Stmt> = body
            .iter()
            .filter(|s| !matches!(s, Stmt::VarDecl(_) | Stmt::VarDecls(_)))
            .cloned()
            .collect();
        if exec.is_empty() {
            self.emit_stmts(body, 0);
            return;
        }
        let step_var = if let Some(ref pname) = self.current_proctype {
            format!("_step_{}", pname)
        } else {
            "_step".to_string()
        };
        let total: usize = exec.iter().map(|s| self.fsize(s)).sum();
        let end = total;
        let mut nodes: Vec<FNode> = Vec::with_capacity(end + 1);
        nodes.resize_with(end + 1, || FNode::Refuse("unreached"));
        nodes[end] = FNode::Halt;

        let mut labels: Vec<(String, usize)> = Vec::new();
        self.flayout_seq(&exec, 0, end, end, &mut nodes, &mut labels);

        // The step number is the index, and taking ownership of each node avoids borrowing
        // the vector while emitting into it.
        #[allow(clippy::needless_range_loop)]
        for i in 0..nodes.len() {
            let node = std::mem::replace(&mut nodes[i], FNode::Refuse("taken"));
            if let FNode::Select { options } = node {
                self.emit(&format!("    -- step {}: selection", i));
                let conds: Vec<String> = options
                    .iter()
                    .map(|(c, _)| match c {
                        Some(e) => self.expr_to_lua(e),
                        None => String::new(),
                    })
                    .collect();
                for (k, (cond, target)) in options.iter().enumerate() {
                    let guard = match cond {
                        Some(e) => {
                            let c = self.expr_to_lua(e);
                            format!(" and ({})", c)
                        }
                        None => {
                            // `else`: enabled only when no other guard is.
                            let others: Vec<String> = conds
                                .iter()
                                .enumerate()
                                .filter(|(j, s)| *j != k && !s.is_empty())
                                .map(|(_, s)| s.clone())
                                .collect();
                            // No condition means either `else` or a guard whose condition is
                            // its first statement (`:: f = 1`), and the AST does not say which.
                            // With no other guard to exclude it is enabled: treating it as a
                            // never-enabled branch made the nested-if branch unreachable, which
                            // was a false clean on []!(f).
                            if others.is_empty() {
                                String::new()
                            } else {
                                format!(" and not (({}))", others.join(") or ("))
                            }
                        }
                    };
                    self.emit("    table.insert(transitions, {");
                    self.indent += 1;
                    self.emit(&format!(
                        "    guard = function(s) return s.{} == {}{} end,",
                        step_var, i, guard
                    ));
                    self.emit(&format!(
                        "    effect = function(s) s.{} = {} end,",
                        step_var, target
                    ));
                    // A selection reads the variables its guards test. Declaring no reads made
                    // every selection look independent of everything, and the search went from
                    // linear to exponential on a chained model: 241 states for 4 tasks became
                    // 19681 for 8.
                    match cond {
                        Some(e) => {
                            let d = self.deps_of(Some(e), &[], &[step_var.as_str()]);
                            self.emit_deps(&d);
                        }
                        None => self.emit_step_deps(&step_var, None),
                    }
                    self.emit(&format!("    label = \"body:step{}\",", i));
                    self.indent -= 1;
                    self.emit("    })");
                }
                continue;
            }

            self.emit("    table.insert(transitions, {");
            self.indent += 1;
            match node {
                FNode::Halt => {
                    self.emit(&format!("    -- step {}: end of body", i));
                    self.emit(&format!(
                        "    guard = function(s) return s.{} == {} end,",
                        step_var, i
                    ));
                    let done = match self.current_proctype {
                        Some(ref p) => format!("s._done_{} = true", p),
                        None => "s._done = true".to_string(),
                    };
                    self.emit(&format!("    effect = function(s) {} end,", done));
                    self.emit_opaque_deps();
                    self.emit(&format!("    label = \"body:step{}\",", i));
                }
                FNode::Simple { stmt, next } => {
                    let (extra_guard, prefix) = self.fstmt_parts(&stmt);
                    self.emit(&format!("    -- step {}: {}", i, describe_stmt(&stmt)));
                    self.emit(&format!(
                        "    guard = function(s) return s.{} == {}{} end,",
                        step_var, i, extra_guard
                    ));
                    self.emit(&format!(
                        "    effect = function(s) {}s.{} = {} end,",
                        prefix, step_var, next
                    ));
                    let is_assert = matches!(stmt, Stmt::Assert(_, _));
                    let mut deps = self.deps_of(None, &[&stmt], &[step_var.as_str()]);
                    if is_assert {
                        deps.write_field("_failed_assert".to_string());
                    }
                    self.emit_deps(&deps);
                    self.emit(&format!("    label = \"body:step{}\",", i));
                }
                FNode::Jump(target) => {
                    self.emit(&format!("    -- step {}: jump {}", i, target));
                    self.emit(&format!(
                        "    guard = function(s) return s.{} == {} end,",
                        step_var, i
                    ));
                    self.emit(&format!(
                        "    effect = function(s) s.{} = {} end,",
                        step_var, target
                    ));
                    self.emit_step_deps(&step_var, None);
                    self.emit(&format!("    label = \"body:step{}\",", i));
                }
                FNode::Goto(name) => {
                    self.emit(&format!("    -- step {}: goto {}", i, name));
                    match labels.iter().find(|(n, _)| *n == name).map(|(_, s)| *s) {
                        Some(t) => {
                            self.emit(&format!(
                                "    guard = function(s) return s.{} == {} end,",
                                step_var, i
                            ));
                            self.emit(&format!(
                                "    effect = function(s) s.{} = {} end,",
                                step_var, t
                            ));
                            self.emit_opaque_deps();
                            self.emit(&format!("    label = \"body:step{}\",", i));
                        }
                        None => {
                            self.emit(&format!(
                                "    guard = function(s) error('goto {} has no label in this proctype') end,",
                                name
                            ));
                            self.emit("    effect = function(s) end,");
                            self.emit_step_deps(&step_var, None);
                        }
                    }
                }
                FNode::Fused {
                    guard,
                    effect,
                    next,
                } => {
                    self.emit(&format!("    -- step {}: fused region", i));
                    self.emit(&format!(
                        "    guard = function(s) return s.{} == {}{} end,",
                        step_var, i, guard
                    ));
                    self.emit(&format!(
                        "    effect = function(s) {}s.{} = {} end,",
                        effect, step_var, next
                    ));
                    self.emit_opaque_deps();
                    self.emit(&format!("    label = \"body:step{}\",", i));
                }
                FNode::Refuse(why) => {
                    self.emit(&format!(
                        "    guard = function(s) error('{} is not supported inside a body') end,",
                        why
                    ));
                    self.emit("    effect = function(s) end,");
                    self.emit_opaque_deps();
                }
                FNode::Select { .. } => unreachable!("handled above"),
            }
            self.indent -= 1;
            self.emit("    })");
        }
    }
}

/// A statement that can be folded into one transition: no control flow of its own.
fn is_simple_stmt(s: &Stmt) -> bool {
    matches!(
        s,
        Stmt::Assignment { .. }
            | Stmt::Skip(_)
            | Stmt::Assert(_, _)
            | Stmt::Expr(_, _)
            | Stmt::Printf(_, _, _)
            | Stmt::Send { .. }
            | Stmt::Recv { .. }
    )
}

fn describe_stmt(s: &Stmt) -> &'static str {
    match s {
        Stmt::Assignment { .. } => "assignment",
        Stmt::Skip(_) => "skip",
        Stmt::Assert(_, _) => "assert",
        Stmt::Expr(_, _) => "wait",
        Stmt::Send { .. } => "send",
        Stmt::Recv { .. } => "recv",
        Stmt::Printf(_, _, _) => "printf",
        _ => "statement",
    }
}
