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
    ///
    /// `exit` is where the selection goes when no option is enabled. Only `for` has one: when its
    /// range is exhausted it continues after the loop. An `if` or a `do` with no enabled guard
    /// blocks, which is what pan calls an invalid end state.
    Select {
        options: Vec<SelectOption>,
        exit: Option<usize>,
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
    /// A rendezvous send: stage the message and stay at this step, so the send completes only
    /// when a receiver takes it. The continuation travels inside the message.
    RdvSend {
        channel: String,
        values: Vec<String>,
        next: usize,
    },
    /// A rendezvous receive: take the staged message, bind the variables, advance this process and
    /// release the stager by writing its continuation into its program counter.
    RdvRecv {
        channel: String,
        vars: Vec<String>,
        next: usize,
    },
    /// The handler of an `unless`: executable while the body is mid-flight, and taking it leaves
    /// the body for `next`.
    UnlessEscape {
        guard: String,
        effect: String,
        from: usize,
        to: usize,
        next: usize,
    },
    /// A construct with no encoding yet. Refused rather than reordered or dropped.
    Refuse(&'static str),
}

/// One branch of a selection.
///
/// Promela lets a guard be an expression or a statement: `:: (x > 0) ->` and `:: c ? x ->` are both
/// legal, and the second is how a receive is written as a guard. The parser reports the first as a
/// condition and the second as a body whose first statement is the guard, so both spellings are
/// carried here and the emitter uses whichever is present.
struct SelectOption {
    condition: Option<Expression>,
    /// The Lua guard derived from the first statement, already in state form.
    stmt_guard: Option<String>,
    /// That first statement, kept so the transition can declare what it reads.
    first_stmt: Option<Stmt>,
    target: usize,
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
            // A block occupies exactly the steps of its statements.
            Stmt::Block(body, _) => body.iter().map(|s| self.fsize(s)).sum::<usize>().max(1),
            // `unless` is the body plus one step for the handler.
            Stmt::Unless { body, handler, .. } => {
                let body_size: usize = body.iter().map(|s| self.fsize(s)).sum();
                let handler_size: usize = handler.iter().map(|s| self.fsize(s)).sum();
                body_size + handler_size
            }
            // `for` is an init, a selection, the body and the update.
            Stmt::For {
                init, update, body, ..
            } => {
                let bodies: usize = body.iter().map(|b| self.fsize(b)).sum();
                self.fsize(init) + 1 + bodies + self.fsize(update)
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
                let _ = is_loop;
                let mut cursor = at + 1;
                let mut options = Vec::new();
                for g in gs {
                    let start = cursor;
                    let (body_cont, body_brk) = if is_loop { (at, next) } else { (next, brk) };
                    cursor = self.flayout_seq(&g.body, start, body_cont, body_brk, nodes, labels);
                    // A guard whose condition the parser did not capture is either `else` or a
                    // statement standing in for the guard. Only a blocking statement supplies one.
                    let stmt_guard = match g.condition {
                        Some(_) => None,
                        None => g.body.first().and_then(|s| self.select_guard_for(s)),
                    };
                    options.push(SelectOption {
                        condition: g.condition.clone(),
                        stmt_guard,
                        first_stmt: g.body.first().cloned(),
                        target: start,
                    });
                }
                // Neither an `if` nor a `do` has an implicit exit: pan reports "invalid end state"
                // for a process sitting at a loop head whose guards are all false, so a `do` that is
                // meant to finish leaves through `break`. Only `for` falls out of its range.
                nodes[at] = FNode::Select {
                    options,
                    exit: None,
                };
            }
            // A capacity-0 channel is a handshake, not a queue: the send and the receive are one
            // transition in pan. It is split into two steps here - stage, then release - because
            // this engine moves one process per transition. See the runtime's `_spin_chan_stage`.
            Stmt::Send { channel, .. } if self.is_rendezvous_send(channel) => {
                nodes[at] = FNode::RdvSend {
                    channel: self.channel_name(channel),
                    values: self.rdv_send_values(s),
                    next,
                }
            }
            Stmt::Recv {
                channel, target, ..
            } if self.is_rendezvous_recv(channel, target) => {
                nodes[at] = FNode::RdvRecv {
                    channel: self.channel_name(channel),
                    vars: self.rdv_recv_vars(target),
                    next,
                }
            }
            // A poll, an eval-receive or a random receive on a capacity-0 channel inspects a staged
            // message instead of taking one. The stage/release encoding has no inspect-only half, and
            // letting the statement fall through to the buffered path made the guard read a queue
            // that does not exist: the receive silently never fired and the claim was refuted where
            // pan accepts. Refused by name instead.
            Stmt::Recv {
                channel, target, ..
            } if self.rendezvous.contains_key(&self.channel_name(channel))
                && !matches!(target, RecvTarget::VarList(_)) =>
            {
                nodes[at] = FNode::Refuse(
                    "a poll or eval receive on a rendezvous channel is not implemented",
                )
            }
            // A receive that binds several variables would need the message as a table; the
            // runtime hands back one value. Refused rather than binding the first and dropping the
            // rest.
            Stmt::Recv {
                target: RecvTarget::VarList(v),
                ..
            } if v.len() > 1 => {
                nodes[at] =
                    FNode::Refuse("a receive that binds more than one variable is not supported")
            }
            // A bare block is a sequence: lay the statements out in place, so the block costs no
            // step and its statements interleave with the surrounding body exactly as if the braces
            // were absent - which is what Promela means by them.
            Stmt::Block(body, _) => {
                self.flayout_seq(body, at, next, brk, nodes, labels);
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
            // pan runs a `for` as an ordinary loop - init, then one body per iteration with the
            // update in between - not as an atomic region. Expanding it to `atomic` at parse time
            // made `for (i : 0 .. 3) { x = i; y = i }` unable to reach x != y, which pan reaches
            // and refutes.
            Stmt::For {
                init,
                condition,
                update,
                body,
                ..
            } => {
                let sel_at = at + self.fsize(init);
                let body_at = sel_at + 1;
                let upd_at = body_at + body.iter().map(|b| self.fsize(b)).sum::<usize>();
                nodes[sel_at] = FNode::Select {
                    options: vec![SelectOption {
                        condition: Some(condition.clone()),
                        stmt_guard: None,
                        first_stmt: None,
                        target: body_at,
                    }],
                    exit: Some(next),
                };
                self.flayout_stmt(init, at, sel_at, brk, nodes, labels);
                // A break in the body leaves the loop, so it targets `next` rather than `brk`.
                self.flayout_seq(body, body_at, upd_at, next, nodes, labels);
                self.flayout_stmt(update, upd_at, sel_at, brk, nodes, labels);
            }
            Stmt::Run(_, _, _) => nodes[at] = FNode::Refuse("run"),
            // `P unless Q`: Q has priority. Every step of the body is disabled exactly where Q is
            // executable, and one transition takes Q and leaves the body. The alternative encoding
            // - Q merely enabled alongside the body - explores runs where the body continues past
            // the escape, which pan does not: a consumer's model refutes under it and holds under
            // pan.
            Stmt::Unless { body, handler, .. } => {
                let body_size: usize = body.iter().map(|s| self.fsize(s)).sum();
                let body_end = at + body_size;
                if handler.len() == 1 && is_simple_stmt(&handler[0]) {
                    self.flayout_seq(body, at, next, brk, nodes, labels);
                    let (g, e) = self.fstmt_parts(&handler[0]);
                    let guard = g.trim_start_matches(" and ").to_string();
                    let guard = if guard.is_empty() {
                        "true".to_string()
                    } else {
                        guard
                    };
                    self.preempt.push((at, body_end, guard.clone()));
                    nodes[body_end] = FNode::UnlessEscape {
                        guard,
                        effect: e,
                        from: at,
                        to: body_end,
                        next,
                    };
                } else {
                    nodes[at] = FNode::Refuse(
                        "an unless handler with more than one statement is not supported",
                    );
                }
            }
            _ => {
                nodes[at] = FNode::Simple {
                    stmt: s.clone(),
                    next,
                }
            }
        }
    }

    /// The conjunction every guard of this body carries, from `provided (expr)`.
    ///
    /// pan runs a proctype only while its `provided` condition holds: the condition gates the
    /// proctype's first step, and since the body is a step sequence the same condition has to hold
    /// at every step. Ignoring it made a process that pan never runs contribute states, which
    /// refuted a model pan proves valid.
    fn provided_guard(&self) -> String {
        match &self.current_provided {
            Some(e) => format!(" and _spin_truthy({})", self.expr_to_lua(e)),
            None => String::new(),
        }
    }

    /// The channel's Promela name, for the rendezvous helpers.
    fn channel_name(&self, channel: &Expression) -> String {
        match channel {
            Expression::Ident(n) => n.clone(),
            Expression::ArrayAccess { name, .. } => name.clone(),
            _ => String::new(),
        }
    }

    /// Is this a send on a capacity-0 channel?
    fn is_rendezvous_send(&self, channel: &Expression) -> bool {
        self.rendezvous.contains_key(&self.channel_name(channel))
    }

    /// Is this a receive that a rendezvous can satisfy? A poll or an eval-receive on a capacity-0
    /// channel keeps its own encoding.
    fn is_rendezvous_recv(&self, channel: &Expression, target: &RecvTarget) -> bool {
        self.rendezvous.contains_key(&self.channel_name(channel))
            && matches!(target, RecvTarget::VarList(_))
    }

    /// The values a rendezvous send carries, already in state form.
    fn rdv_send_values(&self, stmt: &Stmt) -> Vec<String> {
        match stmt {
            Stmt::Send { target, args, .. } => {
                let mut out = vec![match target {
                    SendTarget::Ident(id) => {
                        if self.global_vars.contains(id.as_str()) {
                            format!("s.{}", id)
                        } else if let Some(ref p) = self.current_proctype {
                            format!("s.{}_{}", p, id)
                        } else {
                            format!("s.{}", id)
                        }
                    }
                    SendTarget::Value(v) | SendTarget::Sorted(v) => self.expr_to_lua(v),
                }];
                out.extend(args.iter().map(|a| self.expr_to_lua(a)));
                out
            }
            _ => Vec::new(),
        }
    }

    /// The variables a rendezvous receive binds, in state form.
    fn rdv_recv_vars(&self, target: &RecvTarget) -> Vec<String> {
        match target {
            RecvTarget::VarList(vs) => vs
                .iter()
                .map(|v| {
                    if self.global_vars.contains(v.as_str()) {
                        format!("s.{}", v)
                    } else if let Some(ref p) = self.current_proctype {
                        format!("s.{}_{}", p, v)
                    } else {
                        format!("s.{}", v)
                    }
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The guard a statement supplies when it stands in for a guard.
    ///
    /// Promela lets a receive be written as a guard: `:: c ? x ->` is an option that is enabled
    /// exactly when `c` has a message. The parser reports that as a body whose first statement is
    /// the receive, so the guard has to be derived from it. Any other statement is always
    /// executable and contributes nothing here, which leaves the option as `else`.
    fn select_guard_for(&self, s: &Stmt) -> Option<String> {
        match s {
            Stmt::Send { channel, .. } => Some(format!(
                "not chan_full(s, {})",
                self.channel_to_lua(channel)
            )),
            Stmt::Recv { channel, .. } => Some(format!(
                "not chan_empty(s, {})",
                self.channel_to_lua(channel)
            )),
            Stmt::Expr(e, _) => Some(format!("_spin_truthy({})", self.expr_to_lua(e))),
            _ => None,
        }
    }

    /// The proctype name the stager reports in the message.
    fn rendezvous_owner(&self, _channel: &str) -> String {
        self.current_proctype
            .clone()
            .unwrap_or_else(|| "init".to_string())
    }

    /// Which proctypes can be on the sending side of this channel, and how many instances each
    /// has.
    fn rendezvous_senders(&self, channel: &str) -> Vec<(String, usize)> {
        self.rendezvous.get(channel).cloned().unwrap_or_default()
    }

    /// The field that terminates this body's process.
    fn done_field(&self) -> String {
        self.current_done
            .clone()
            .unwrap_or_else(|| "_done".to_string())
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
                let v = self.wrap_value(target, self.expr_to_lua(value));
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
            Stmt::Expr(e, _) => (
                format!(" and _spin_truthy({})", self.expr_to_lua(e)),
                String::new(),
            ),
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
                    format!(" and not chan_full(s, {})", chan),
                    format!("_spin_chan_send(s, {}, {}); ", chan, values.join(", ")),
                )
            }
            Stmt::Recv {
                channel, target, ..
            } => {
                let chan = self.channel_to_lua(channel);
                let field = |v: &str| {
                    if self.global_vars.contains(v) {
                        v.to_string()
                    } else if let Some(ref p) = self.current_proctype {
                        format!("{}_{}", p, v)
                    } else {
                        v.to_string()
                    }
                };
                // `_spin_chan_recv` *returns* the message; the old emission passed the target as a
                // third argument and dropped the return value, so `c ? x` left x untouched - a silent
                // wrong in the direction of any assertion about what was received.
                match target {
                    RecvTarget::VarList(v) if v.len() == 1 => (
                        format!(" and not chan_empty(s, {})", chan),
                        format!(
                            "local _v = _spin_chan_recv(s, {}); s.{} = _v; ",
                            chan,
                            field(&v[0])
                        ),
                    ),
                    RecvTarget::Random(v) if v.len() == 1 => (
                        format!(" and not chan_empty(s, {})", chan),
                        format!(
                            "local _v = _spin_chan_recv(s, {}); s.{} = _v; ",
                            chan,
                            field(&v[0])
                        ),
                    ),
                    // A poll inspects the head without taking it, so it has a guard and no effect.
                    RecvTarget::Poll(e) => (
                        format!(
                            " and _spin_chan_poll(s, {}, {}) ~= nil",
                            chan,
                            self.expr_to_lua(e)
                        ),
                        String::new(),
                    ),
                    // An eval-receive inspects the head; it is the statement emitter's job to take
                    // the message, and a bare `c ? [v]` as a step takes nothing.
                    RecvTarget::Eval(e) => (
                        format!(
                            " and _spin_chan_poll(s, {}, {}) ~= nil",
                            chan,
                            self.expr_to_lua(e)
                        ),
                        String::new(),
                    ),
                    _ => (String::new(), String::new()),
                }
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
        let step_var = self
            .current_step
            .clone()
            .unwrap_or_else(|| "_step".to_string());
        self.preempt.clear();
        let provided = self.provided_guard();
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
            // A body step inside an `unless` is disabled exactly where the handler is
            // executable: pan gives the escape priority, so the body may not move.
            let preempt: String = self
                .preempt
                .iter()
                .filter(|(from, to, _)| i >= *from && i < *to)
                .map(|(_, _, g)| format!(" and not ({})", g))
                .collect();
            let provided = format!("{}{}", provided, preempt);
            if let FNode::Select { options, exit } = node {
                self.emit(&format!("    -- step {}: selection", i));
                // Each option's guard text, in the order the options appear. A condition is an
                // expression; a statement guard is whatever makes that statement executable, such
                // as a non-empty channel for a receive. An option with neither is `else`.
                // Each option's guard text, already a Lua boolean. A condition is a Promela
                // expression and has to be read with Promela truthiness, where 0 is false; a
                // statement guard is a boolean already; an option with neither is `else`. The
                // texts are used both positively and negated (for `else` and for a loop's exit),
                // so they must agree on what "true" means.
                let conds: Vec<String> = options
                    .iter()
                    .map(|o| match (&o.condition, &o.stmt_guard) {
                        (Some(e), _) => format!("_spin_truthy({})", self.expr_to_lua(e)),
                        (None, Some(g)) => g.clone(),
                        (None, None) => String::new(),
                    })
                    .collect();
                for (k, option) in options.iter().enumerate() {
                    let cond = option.condition.clone();
                    let target = option.target;
                    let guard = if !conds[k].is_empty() {
                        format!(" and ({})", conds[k])
                    } else {
                        // `else`: enabled only when no other guard is. With no other guard to
                        // exclude it, it is enabled - treating it as a never-enabled branch made
                        // the nested-if branch unreachable, which was a false clean on []!(f).
                        let others: Vec<String> = conds
                            .iter()
                            .enumerate()
                            .filter(|(j, s)| *j != k && !s.is_empty())
                            .map(|(_, s)| s.clone())
                            .collect();
                        if others.is_empty() {
                            String::new()
                        } else {
                            format!(" and not (({}))", others.join(") or ("))
                        }
                    };
                    self.emit("    table.insert(transitions, {");
                    self.indent += 1;
                    self.emit(&format!(
                        "    guard = function(s) return s.{} == {}{}{} end,",
                        step_var, i, guard, provided
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
                            let d = self.deps_of(Some(&e), &[], &[step_var.as_str()]);
                            self.emit_deps(&d);
                        }
                        None => {
                            // A statement guard reads whatever the statement guards on: the
                            // channel for a receive, the expression for a blocking one.
                            match option.first_stmt.as_ref() {
                                Some(st) => {
                                    let d = self.deps_of(None, &[st], &[step_var.as_str()]);
                                    self.emit_deps(&d);
                                }
                                None => self.emit_step_deps(&step_var, None),
                            }
                        }
                    }
                    self.emit(&format!("    label = \"body:step{}\",", i));
                    self.indent -= 1;
                    self.emit("    })");
                }
                // A loop ends when none of its guards is enabled: pan reaches the exit, it does
                // not block. The transition is the negation of every option guard, so it is
                // enabled exactly where the selection has nothing to choose.
                if let Some(target) = exit {
                    let live: Vec<String> =
                        conds.iter().filter(|s| !s.is_empty()).cloned().collect();
                    let guard = if live.is_empty() {
                        String::new()
                    } else {
                        format!(" and not (({}))", live.join(") or ("))
                    };
                    self.emit("    table.insert(transitions, {");
                    self.indent += 1;
                    self.emit(&format!(
                        "    guard = function(s) return s.{} == {}{}{} end,",
                        step_var, i, guard, provided
                    ));
                    self.emit(&format!(
                        "    effect = function(s) s.{} = {} end,",
                        step_var, target
                    ));
                    let mut deps: Option<super::deps::Deps> = None;
                    for option in &options {
                        if let Some(e) = &option.condition {
                            deps = Some(match deps {
                                None => self.deps_of(Some(e), &[], &[step_var.as_str()]),
                                Some(mut d) => {
                                    let d2 = self.deps_of(Some(e), &[], &[step_var.as_str()]);
                                    d.merge(&d2);
                                    d
                                }
                            });
                        }
                    }
                    match deps {
                        Some(d) => self.emit_deps(&d),
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
                        "    guard = function(s) return s.{} == {}{} end,",
                        step_var, i, provided
                    ));
                    let done = format!("s.{} = true", self.done_field());
                    self.emit(&format!("    effect = function(s) {} end,", done));
                    self.emit_opaque_deps();
                    self.emit(&format!("    label = \"body:step{}\",", i));
                }
                FNode::Simple { stmt, next } => {
                    let (extra_guard, prefix) = self.fstmt_parts(&stmt);
                    // Falling off the end of the body ends the process directly. Advancing to the
                    // halt step first costs a step per process, and every extra step is another
                    // interleaving for the search to enumerate.
                    let done_field = self.done_field();
                    let tail = if next == end {
                        format!("s.{} = true", done_field)
                    } else {
                        format!("s.{} = {}", step_var, next)
                    };
                    self.emit(&format!("    -- step {}: {}", i, describe_stmt(&stmt)));
                    self.emit(&format!(
                        "    guard = function(s) return s.{} == {}{}{} end,",
                        step_var, i, extra_guard, provided
                    ));
                    self.emit(&format!("    effect = function(s) {}{} end,", prefix, tail));
                    let is_assert = matches!(stmt, Stmt::Assert(_, _));
                    // The step counter is read by the guard and the termination flag is written
                    // when this is the last step of the body; both are declared, the step counter
                    // by the same convention as `emit_step_deps`.
                    let mut deps = self.deps_of(None, &[&stmt], &[step_var.as_str()]);
                    if next == end {
                        deps.write_field(done_field.clone());
                    }
                    if is_assert {
                        deps.write_field("_failed_assert".to_string());
                    }
                    self.emit_deps(&deps);
                    self.emit(&format!("    label = \"body:step{}\",", i));
                }
                FNode::Jump(target) => {
                    self.emit(&format!("    -- step {}: jump {}", i, target));
                    self.emit(&format!(
                        "    guard = function(s) return s.{} == {}{} end,",
                        step_var, i, provided
                    ));
                    if target == end {
                        let done = self.done_field();
                        self.emit(&format!("    effect = function(s) s.{} = true end,", done));
                        self.emit_step_deps(&step_var, Some(done));
                    } else {
                        self.emit(&format!(
                            "    effect = function(s) s.{} = {} end,",
                            step_var, target
                        ));
                        self.emit_step_deps(&step_var, None);
                    }
                    self.emit(&format!("    label = \"body:step{}\",", i));
                }
                FNode::Goto(name) => {
                    self.emit(&format!("    -- step {}: goto {}", i, name));
                    match labels.iter().find(|(n, _)| *n == name).map(|(_, s)| *s) {
                        Some(t) => {
                            self.emit(&format!(
                                "    guard = function(s) return s.{} == {}{} end,",
                                step_var, i, provided
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
                        "    guard = function(s) return s.{} == {}{}{} end,",
                        step_var, i, guard, provided
                    ));
                    self.emit(&format!(
                        "    effect = function(s) {}s.{} = {} end,",
                        effect, step_var, next
                    ));
                    self.emit_opaque_deps();
                    self.emit(&format!("    label = \"body:step{}\",", i));
                }
                FNode::RdvSend {
                    channel,
                    values,
                    next,
                } => {
                    // Stage and stay put. The guard rejects a second stage, so the process blocks
                    // here until a receiver takes it - which is what a capacity-0 send means.
                    self.emit(&format!(
                        "    -- step {}: rendezvous send on {}",
                        i, channel
                    ));
                    self.emit(&format!(
                        "    guard = function(s) return s.{} == {} and not _spin_chan_staged(s, '{}') end,",
                        step_var, i, channel
                    ));
                    let mut args = vec![
                        "s".to_string(),
                        format!("'{}'", channel),
                        format!("'{}'", self.rendezvous_owner(&channel)),
                        "_spin_pid".to_string(),
                        next.to_string(),
                    ];
                    args.extend(values);
                    self.emit(&format!(
                        "    effect = function(s) _spin_chan_stage({}) end,",
                        args.join(", ")
                    ));
                    // It reads the channel and writes it; it does not touch the program counter,
                    // because completing the send is the receiver's doing.
                    let mut deps = self.deps_of(None, &[], &[]);
                    deps.write_field(channel.clone());
                    deps.reads.insert(channel.clone());
                    self.emit_deps(&deps);
                    self.emit(&format!("    label = \"body:step{}\",", i));
                }
                FNode::RdvRecv {
                    channel,
                    vars,
                    next,
                } => {
                    self.emit(&format!(
                        "    -- step {}: rendezvous receive on {}",
                        i, channel
                    ));
                    self.emit(&format!(
                        "    guard = function(s) return s.{} == {} and _spin_chan_staged(s, '{}') end,",
                        step_var, i, channel
                    ));
                    let mut effect = format!("local _m = _spin_chan_pair(s, '{}'); ", channel);
                    for (k, var) in vars.iter().enumerate() {
                        effect.push_str(&format!("{} = _m.values[{}]; ", var, k + 1));
                    }
                    effect.push_str(&format!("s.{} = {}; ", step_var, next));
                    // Release the stager: its continuation travelled in the message, and its
                    // program counter is the only thing that makes the send complete.
                    for (prefix, instances) in self.rendezvous_senders(&channel) {
                        for instance in 0..instances {
                            let stager = if instances == 1 {
                                prefix.clone()
                            } else {
                                format!("{}{}", prefix, instance)
                            };
                            effect.push_str(&format!(
                                "if _m.owner == '{}' and _m.pid == {} then s._step_{} = _m.cont end; ",
                                prefix.trim_end_matches('_'),
                                instance,
                                stager
                            ));
                        }
                    }
                    self.emit(&format!("    effect = function(s) {} end,", effect));
                    let mut deps = self.deps_of(None, &[], &[step_var.as_str()]);
                    deps.reads.insert(channel.clone());
                    deps.write_field(channel.clone());
                    for var in &vars {
                        if let Some(field) = var.strip_prefix("s.") {
                            deps.write_field(field.to_string());
                        }
                    }
                    // The stager's counter is written here, which no dependency model of one
                    // process can see; declared so reduction cannot prune the handshake.
                    for (prefix, instances) in self.rendezvous_senders(&channel) {
                        for instance in 0..instances {
                            let stager = if instances == 1 {
                                prefix.clone()
                            } else {
                                format!("{}{}", prefix, instance)
                            };
                            deps.write_field(format!("_step_{}", stager));
                        }
                    }
                    self.emit_deps(&deps);
                    self.emit(&format!("    label = \"body:step{}\",", i));
                }
                FNode::UnlessEscape {
                    guard,
                    effect,
                    from,
                    to,
                    next,
                } => {
                    self.emit(&format!(
                        "    -- step {}: unless escape for steps {}..{}",
                        i, from, to
                    ));
                    self.emit(&format!(
                        "    guard = function(s) return s.{} >= {} and s.{} < {} and ({}) end,",
                        step_var, from, step_var, to, guard
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
