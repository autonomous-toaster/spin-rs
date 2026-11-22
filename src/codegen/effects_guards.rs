use super::LuaGenerator;
use crate::parser::ast::*;

impl LuaGenerator {
    /// Emit a proctype body as an ordered sequence of steps.
    ///
    /// A body of N statements used to emit N transitions that were all enabled at once, so the
    /// statements ran in any order: `b = 1; a = 1` could set `a` first and refute a model pan proves
    /// valid. A body longer than one statement now goes through the step machine, which is the one
    /// place in this generator that sequences correctly, and a trailing `break` makes the last
    /// step terminate the process instead of leaving it re-enabled forever.
    pub(crate) fn emit_body_sequenced(&mut self, body: &[Stmt], depth: usize) {
        let mut exec: Vec<Stmt> = body
            .iter()
            .filter(|s| !matches!(s, Stmt::VarDecl(_) | Stmt::VarDecls(_)))
            .cloned()
            .collect();
        if exec.len() <= 1 {
            self.emit_stmts(body, depth);
            return;
        }
        exec.push(Stmt::Break(0));
        self.emit_guards(
            "body",
            &[Guard {
                condition: None,
                body: exec,
                line: 0,
            }],
            depth,
        );
    }

    pub(crate) fn emit_guards(&mut self, kind: &str, guards: &[Guard], _depth: usize) {
        // Detect multi-statement guard bodies — need step-counter for interleaving.
        // Single-statement guards can use the simple flat approach.
        let has_multi_stmt: bool = guards.iter().any(|g| {
            g.body
                .iter()
                .filter(|s| {
                    if matches!(s, Stmt::Skip(_)) {
                        return false;
                    }
                    if let Stmt::Expr(e, _) = s {
                        if let Expression::FuncCall { name, .. } = e {
                            return self.inlines.contains_key(name.as_str());
                        }
                        // A blocking expression is a statement in its own right. Counting it
                        // here is what stops the flat path from hoisting it in front of the
                        // writes that precede it.
                        return true;
                    }
                    true
                })
                .count()
                > 1
        });

        if !has_multi_stmt {
            // Check if there are any inline calls — need step-counter body steps for those
            let has_inline_calls: bool = guards.iter().any(|g| {
                g.body.iter().any(|s| {
                    if let Stmt::Expr(e, _) = s
                        && let Expression::FuncCall { name, .. } = e
                    {
                        return self.inlines.contains_key(name.as_str());
                    }
                    false
                })
            });
            if !has_inline_calls {
                return self.emit_guards_flat(kind, guards);
            }
        }

        // Multi-statement guard bodies: use step-counter for proper interleaving.
        // Step 0: guard selection, Steps 1..N: body, last step loops to 0.
        let step_var = if let Some(ref pname) = self.current_proctype {
            format!("_step_{}", pname)
        } else {
            "_step".to_string()
        };

        let init_cond: Vec<String> = guards
            .iter()
            .map(|g| match &g.condition {
                Some(e) => self.expr_to_lua(e),
                None => "true".to_string(),
            })
            .collect();

        // Step 0: guard selection — one transition per guard
        let mut body_steps: Vec<(usize, &Stmt)> = Vec::new();
        let mut step_counter: usize = 1;

        for (gi, guard) in guards.iter().enumerate() {
            let gc = if guard.condition.is_none() {
                let others: Vec<&str> = init_cond
                    .iter()
                    .enumerate()
                    .filter(|(j, s)| *j != gi && s.as_str() != "true")
                    .map(|(_, s)| s.as_str())
                    .collect();
                if others.is_empty() {
                    "true".to_string()
                } else if others.len() == 1 {
                    format!("not ({})", others[0])
                } else {
                    format!("not ({})", others.join(" or "))
                }
            } else {
                init_cond[gi].clone()
            };

            // Blocking expressions are NOT merged into the selection guard. Merging them
            // moved a wait that happens after a write to before it: for
            //     do :: (1) -> ready = 1; (peer_ready); go = 1 od
            // the selection guard became (1) and (peer_ready), so the process blocked before
            // setting ready and the model could deadlock where spin explores a path. They are
            // emitted as their own guarded steps below instead.
            let sel_cond = gc.clone();

            let body_non_expr: Vec<&Stmt> = guard
                .body
                .iter()
                .filter(|s| {
                    if matches!(s, Stmt::Skip(_)) {
                        return false;
                    }
                    // Include inline function calls as body steps
                    if let Stmt::Expr(e, _) = s {
                        if let Expression::FuncCall { name, .. } = e {
                            return self.inlines.contains_key(name.as_str());
                        }
                        // Non-inline: a blocking wait, which is a step of its own.
                        return true;
                    }
                    true
                })
                .collect();

            let target_step = if body_non_expr.is_empty() {
                0
            } else {
                let base = step_counter;
                for bs in &body_non_expr {
                    body_steps.push((step_counter, bs));
                    step_counter += 1;
                }
                base
            };

            self.emit(&format!("    -- {} guard {} (sel): {}", kind, gi, sel_cond));
            self.emit("    table.insert(transitions, {");
            self.indent += 1;
            self.emit(&format!(
                "    guard = function(s) return s.{} == 0 and ({}) end,",
                step_var, sel_cond
            ));
            self.emit(&format!(
                "    effect = function(s) s.{} = {} end,",
                step_var, target_step
            ));
            // The selection step only evaluates the guard and moves the counter; it executes
            // none of the body, so the body's writes must not be attributed to it.
            let readings: Vec<Expression> = guard.condition.iter().cloned().collect();
            let mut deps = {
                let refs: Vec<&Expression> = readings.iter().collect();
                self.deps_selection(&refs, &[step_var.as_str()])
            };
            {
                let map = self.map_fn_pub();
                for st in &guard.body {
                    if let Stmt::Expr(e, _) = st {
                        deps.read_expr(e, &map);
                    }
                }
            }
            self.emit_deps(&deps);
            self.emit(&format!("    label = \"{}:sel{}\",", kind, gi));
            self.indent -= 1;
            self.emit("    })");
        }

        // A trailing `break` is just "this proctype is done". Emitting it as its own
        // transition makes it independent of everything else, so it can be deferred past the
        // next proctype's start: every such choice doubles the reachable state space, which
        // made a linear chain of tasks 2^N states instead of ~5 per task. Fold it into the
        // preceding step, which is what spin does by folding termination into the last
        // statement. A body that is only `break` keeps its own step: there is nothing to fold
        // into, and it is degenerate enough not to matter.
        let mut trailing_break = false;
        if body_steps.len() > 1 && matches!(body_steps.last(), Some((_, Stmt::Break(_)))) {
            body_steps.pop();
            trailing_break = true;
        }

        // Steps 1..N: emit one transition per body statement
        let total = body_steps.len();
        for (idx, (step, s)) in body_steps.iter().enumerate() {
            let is_last = idx + 1 >= total;
            let next = if is_last { 0 } else { step + 1 };
            // Tail appended to this step's effect: either advance the step counter, or mark
            // the proctype done when it is the folded trailing break.
            let tail = if is_last && trailing_break {
                match self.current_proctype {
                    Some(ref pname) => format!("; s._done_{} = true", pname),
                    None => String::new(),
                }
            } else {
                format!("; s.{} = {}", step_var, next)
            };

            // Inline calls: emit_inline_body produces the full transition
            if let Stmt::Expr(e, _) = s
                && let Expression::FuncCall { name, args } = e
                && let Some(inline_def) = self.inlines.get(name.as_str()).cloned()
            {
                self.emit(&format!(
                    "    -- {} body step {} (inline: {})",
                    kind, step, name
                ));
                let subs: Vec<(String, String)> = inline_def
                    .parameters
                    .iter()
                    .zip(args.iter())
                    .map(|(p, a)| (p.clone(), self.expr_to_lua(a)))
                    .collect();
                self.emit("    table.insert(transitions, {");
                self.indent += 1;
                self.emit_opaque_deps();
                self.emit_inline_body(&inline_def, &subs, step_var.as_str(), *step, next);
                self.indent -= 1;
                self.emit("    })");
                continue;
            }

            // Nested control flow inside a guard body used to fall through to the
            // catch-all arm below, which emitted only "advance the step counter" and
            // silently discarded the block. A model whose only assignment to a variable
            // sat inside a nested if therefore never set it, and a property it violates
            // was reported clean.
            if let Stmt::If(inner) = s {
                self.emit(&format!("    -- {} body step {} (nested if)", kind, step));
                self.emit_nested_if_choice(inner, step_var.as_str(), *step, next);
                continue;
            }
            if matches!(s, Stmt::Do(_)) {
                // A nested do/od needs its own selection loop, which this step machine
                // cannot express yet. Fail loudly rather than drop the block.
                self.emit(&format!("    -- {} body step {} (nested do)", kind, step));
                self.emit("    table.insert(transitions, {");
                self.emit_opaque_deps();
                self.emit("        guard = function(s) error('nested do/od inside a guard body is not supported') end,");
                self.emit("        effect = function(s) end,");
                self.emit("    })");
                continue;
            }

            self.emit(&format!("    -- {} body step {}", kind, step));
            self.emit("    table.insert(transitions, {");
            self.indent += 1;
            self.emit(&format!(
                "    guard = function(s) return s.{} == {} end,",
                step_var, step
            ));

            match s {
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
                        let idx_str = self.expr_to_lua(idx);
                        format!("s.{}[{} + 1]", target_name, idx_str)
                    } else {
                        format!("s.{}", target_name)
                    };
                    self.emit(&format!(
                        "    effect = function(s) {} = {}{} end,",
                        target_expr, v, tail
                    ));
                }
                Stmt::Break(_) => {
                    let done_flag = if let Some(ref pname) = self.current_proctype {
                        format!("s._done_{}", pname)
                    } else {
                        continue;
                    };
                    self.emit(&format!(
                        "    effect = function(s) {} = true end,",
                        done_flag
                    ));
                }
                Stmt::Goto(_, _) => {
                    // Advancing past a goto would run the statements it jumps over and skip the
                    // ones it jumps to, silently. Refuse until the jump has a real edge target.
                    self.emit(
                        "    guard = function(s) error('goto is not supported as a step inside a \
                         sequence: it has no edge target yet') end,",
                    );
                    self.emit("    effect = function(s) end,");
                }
                Stmt::Expr(e, _) => {
                    // A blocking wait: enabled only while the condition holds, and it changes
                    // nothing but the step counter.
                    let cond = self.expr_to_lua(e);
                    self.emit(&format!(
                        "    guard = function(s) return s.{} == {} and ({}) end,",
                        step_var, step, cond
                    ));
                    self.emit(&format!(
                        "    effect = function(s) {} end,",
                        tail.trim_start_matches("; ")
                    ));
                }
                Stmt::Assert(expr, _) => {
                    // The step is always taken; the condition decides whether the state is marked
                    // as assert-failed. Making the transition conditional instead only reported a
                    // failure when the whole model stopped.
                    let e = self.expr_to_lua(expr);
                    let check = self.assert_failure_effect(&e, &format!("assert({})", e));
                    self.emit(&format!(
                        "    effect = function(s) {}; {} end,",
                        check,
                        tail.trim_start_matches("; ")
                    ));
                }
                Stmt::Send {
                    channel, target, ..
                } => {
                    let id = match target {
                        SendTarget::Ident(id) => id.clone(),
                        SendTarget::Value(_) => "expr".to_string(),
                        SendTarget::Sorted(val) => self.expr_to_lua(val),
                    };
                    self.emit(&format!(
                        "    effect = function(s) chan_send({}, {}, ...); s.{} = {} end,",
                        self.channel_to_lua(channel),
                        id,
                        step_var,
                        next
                    ));
                }
                Stmt::Recv {
                    channel, target, ..
                } => {
                    if let RecvTarget::VarList(vars) = target {
                        let vs = vars.join(", ");
                        self.emit(&format!(
                            "    effect = function(s) chan_recv({}, {}); s.{} = {} end,",
                            self.channel_to_lua(channel),
                            vs,
                            step_var,
                            next
                        ));
                    }
                }
                _ => {
                    // Anything else would be advanced past with no effect, which silently drops
                    // the statement: an atomic block, a for loop or an unless handler would all
                    // vanish, and a model whose only write sat inside one would come back clean.
                    // Refuse the step instead, so the run reports "could not be checked".
                    self.emit(&format!(
                        "    guard = function(s) error('{} is not supported as a step inside a \
                         sequence') end,",
                        Self::stmt_kind_name(s)
                    ));
                    self.emit("    effect = function(s) end,");
                }
            }
            // The step counter is always moved; the done flag is written only on break, and
            // declaring it either way over-approximates rather than under-approximates.
            let mut deps = self.deps_of(None, &[*s], &[step_var.as_str()]);
            if let Some(ref pname) = self.current_proctype {
                let done = format!("_done_{}", pname);
                deps.write_field(done);
            }
            if matches!(s, Stmt::Assert(_, _)) {
                // A model-level finding, so no claim can reduce it away.
                deps.write_field("_failed_assert".to_string());
            }
            self.emit_deps(&deps);
            self.emit(&format!("    label = \"{}:step{}\",", kind, step));
            self.indent -= 1;
            self.emit("    })");
        }
    }

    /// The name of a statement kind, for a diagnostic that names what it refused.
    fn stmt_kind_name(s: &Stmt) -> &'static str {
        match s {
            Stmt::Atomic(_, _) => "atomic",
            Stmt::DStep(_, _) => "d_step",
            Stmt::Unless { .. } => "unless",
            Stmt::For { .. } => "for",
            Stmt::If(_) => "if",
            Stmt::Do(_) => "do",
            Stmt::Label(_, _) => "label",
            Stmt::Printf(_, _, _) => "printf",
            Stmt::Run(_, _, _) => "run",
            Stmt::Recv { .. } => "recv",
            Stmt::VarDecl(_) | Stmt::VarDecls(_) => "declaration",
            _ => "statement",
        }
    }

    /// Expand a nested `if` inside a guard body into one transition per branch.
    ///
    /// Each branch runs from `step` and continues at `next`, which is what the flat
    /// emitter does for a top-level if. A branch that contains further nested control
    /// flow, or anything whose effect cannot be expressed in one step, emits a transition
    /// that raises at evaluation time: the run then reports "could not be checked" instead
    /// of silently dropping the block.
    fn emit_nested_if_choice(
        &mut self,
        guards: &[Guard],
        step_var: &str,
        step: usize,
        next: usize,
    ) {
        // A branch may only be folded into this step when it has at most one effectful
        // statement. Folding several would run them atomically, removing the intermediate
        // states between them and hiding a violation: a branch "f = 1; f = 0" would leave
        // f permanently 0, so []!(f) came back clean while pan refutes it. Report such a
        // branch instead of quietly under-approximating.
        let effectful = |g: &Guard| {
            g.body
                .iter()
                .filter(|s| match s {
                    Stmt::Skip(_) => false,
                    Stmt::Expr(e, _) => match e {
                        Expression::FuncCall { name, .. } => {
                            self.inlines.contains_key(name.as_str())
                        }
                        _ => false,
                    },
                    _ => true,
                })
                .count()
        };
        if guards.iter().any(|g| effectful(g) > 1) {
            self.emit("    -- nested if with a multi-statement branch: refusing to fold it into");
            self.emit("    -- one step, which would run the statements atomically and hide the");
            self.emit("    -- states between them");
            self.emit("    table.insert(transitions, {");
            self.emit_opaque_deps();
            self.emit(
                "        guard = function(s) error('multi-statement branch in a nested if inside a guard body is not supported') end,",
            );
            self.emit("        effect = function(s) end,");
            self.emit("    })");
            return;
        }

        let cond_strs: Vec<String> = guards
            .iter()
            .map(|g| match &g.condition {
                Some(e) => self.expr_to_lua(e),
                None => "true".to_string(),
            })
            .collect();

        for (gi, g) in guards.iter().enumerate() {
            let mut cond = if g.condition.is_none() {
                let others: Vec<&str> = cond_strs
                    .iter()
                    .enumerate()
                    .filter(|(j, s)| *j != gi && s.as_str() != "true")
                    .map(|(_, s)| s.as_str())
                    .collect();
                if others.is_empty() {
                    "true".to_string()
                } else {
                    format!("not ({})", others.join(" or "))
                }
            } else {
                cond_strs[gi].clone()
            };

            // Blocking expressions in the branch body become part of the guard.
            let blocking: Vec<String> = g
                .body
                .iter()
                .filter_map(|s| match s {
                    Stmt::Expr(e, _) => {
                        if let Expression::FuncCall { name, .. } = e
                            && self.inlines.contains_key(name.as_str())
                        {
                            return None;
                        }
                        Some(self.expr_to_lua(e))
                    }
                    _ => None,
                })
                .collect();
            if !blocking.is_empty() {
                let extra = if blocking.len() == 1 {
                    blocking[0].clone()
                } else {
                    format!("({})", blocking.join(" and "))
                };
                cond = if cond == "true" {
                    extra
                } else {
                    format!("({}) and {}", cond, extra)
                };
            }

            let mut effects: Vec<String> = Vec::new();
            let mut guards_extra: Vec<String> = Vec::new();
            let mut terminates = false;
            let mut supported = true;
            for st in &g.body {
                match st {
                    Stmt::Skip(_) => {}
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
                        if let Some(idx) = index {
                            let idx_str = self.expr_to_lua(idx);
                            effects.push(format!("s.{}[{} + 1] = {}", target_name, idx_str, v));
                        } else {
                            effects.push(format!("s.{} = {}", target_name, v));
                        }
                    }
                    Stmt::Break(_) => match self.current_proctype {
                        Some(ref pname) => {
                            effects.push(format!("s._done_{} = true", pname));
                            terminates = true;
                        }
                        None => supported = false,
                    },
                    Stmt::Goto(_, _) => {}
                    Stmt::Assert(e, _) => guards_extra.push(self.expr_to_lua(e)),
                    _ => supported = false,
                }
            }

            self.emit("    table.insert(transitions, {");
            self.indent += 1;
            if !supported {
                self.emit(
                    "    guard = function(s) error('unsupported statement inside a nested if in a guard body') end,",
                );
                self.emit("    effect = function(s) end,");
                self.indent -= 1;
                self.emit("    })");
                return;
            }
            let mut all_conds: Vec<String> = vec![format!("s.{} == {}", step_var, step)];
            if cond != "true" {
                all_conds.push(cond);
            }
            all_conds.extend(guards_extra);
            self.emit(&format!(
                "    guard = function(s) return {} end,",
                all_conds.join(" and ")
            ));
            if !terminates {
                effects.push(format!("s.{} = {}", step_var, next));
            }
            self.emit(&format!(
                "    effect = function(s) {} end,",
                effects.join("; ")
            ));
            {
                let body_refs: Vec<&Stmt> = g.body.iter().collect();
                let mut deps = self.deps_of(g.condition.as_ref(), &body_refs, &[step_var]);
                if let Some(ref pname) = self.current_proctype {
                    let done = format!("_done_{}", pname);
                    deps.write_field(done);
                }
                self.emit_deps(&deps);
            }
            self.emit(&format!("    label = \"if:branch{}\",", gi));
            self.indent -= 1;
            self.emit("    })");
        }
    }
    /// Emit inline body statements as atomic transitions within a guard body step.
    fn emit_inline_body(
        &mut self,
        inline_def: &crate::parser::ast::InlineDef,
        subs: &[(String, String)],
        step_var: &str,
        current_step: usize,
        next_step: usize,
    ) {
        for s in &inline_def.body {
            match s {
                Stmt::Atomic(body, _) | Stmt::DStep(body, _) => {
                    // Atomic inline body: all statements execute atomically together
                    // as one transition. Collect guards and effects.
                    let mut guard_parts: Vec<String> = Vec::new();
                    let mut effects: Vec<String> = Vec::new();

                    for inner in body {
                        match inner {
                            Stmt::Expr(e, _) => {
                                let e_str = self.substitute_expr(e, subs);
                                guard_parts.push(e_str);
                            }
                            Stmt::Assignment {
                                target,
                                index,
                                value,
                                ..
                            } => {
                                let v = self.substitute_expr(value, subs);
                                let t = self.substitute_var(target, subs);
                                let target_name = if self.global_vars.contains(t.as_str()) {
                                    t
                                } else if let Some(ref pname) = self.current_proctype {
                                    format!("{}_{}", pname, t)
                                } else {
                                    t
                                };
                                if let Some(idx) = index {
                                    let idx_str = self.substitute_expr(idx, subs);
                                    effects.push(format!(
                                        "s.{}[{} + 1] = {}",
                                        target_name, idx_str, v
                                    ));
                                } else {
                                    effects.push(format!("s.{} = {}", target_name, v));
                                }
                            }
                            _ => {}
                        }
                    }

                    let additional_guard = if guard_parts.is_empty() {
                        String::new()
                    } else if guard_parts.len() == 1 {
                        format!(" and {}", guard_parts[0])
                    } else {
                        format!(" and ({})", guard_parts.join(" and "))
                    };

                    let effect_str = if effects.is_empty() {
                        format!("s.{} = {}", step_var, next_step)
                    } else {
                        format!("{}; s.{} = {}", effects.join("; "), step_var, next_step)
                    };

                    self.emit(&format!(
                        "    guard = function(s) return s.{} == {}{} end,",
                        step_var, current_step, additional_guard
                    ));
                    self.emit(&format!("    effect = function(s) {} end,", effect_str));
                    self.emit(&format!("    label = \"inline:{}\",", inline_def.name));
                }
                _ => {
                    // Non-atomic inline statements handled by emit_inline_stmt_with_subs in stmts.rs
                }
            }
        }
    }

    /// Simple flat guard emission: one transition per guard, all body stmts atomically in effect.
    /// Blocking expressions (Stmt::Expr) are merged into the guard condition.
    /// Array element assignments use the correct Lua table indexing.
    fn emit_guards_flat(&mut self, kind: &str, guards: &[Guard]) {
        let cond_strs: Vec<String> = guards
            .iter()
            .map(|g| match &g.condition {
                Some(e) => self.expr_to_lua(e),
                None => "true".to_string(),
            })
            .collect();

        for (i, guard) in guards.iter().enumerate() {
            let mut cond_str = if guard.condition.is_none() {
                let others: Vec<&str> = cond_strs
                    .iter()
                    .enumerate()
                    .filter(|(j, s)| *j != i && s.as_str() != "true")
                    .map(|(_, s)| s.as_str())
                    .collect();
                if others.is_empty() {
                    "true".to_string()
                } else if others.len() == 1 {
                    format!("not ({})", others[0])
                } else {
                    format!("not ({})", others.join(" or "))
                }
            } else {
                cond_strs[i].clone()
            };

            // Merge Stmt::Expr (blocking conditions) from guard body into guard
            let blocking: Vec<String> = guard
                .body
                .iter()
                .filter_map(|s| match s {
                    Stmt::Expr(e, _) => Some(self.expr_to_lua(e)),
                    _ => None,
                })
                .collect();
            if !blocking.is_empty() {
                let extra = if blocking.len() == 1 {
                    blocking[0].clone()
                } else {
                    format!("({})", blocking.join(" and "))
                };
                if cond_str == "true" {
                    cond_str = extra;
                } else {
                    cond_str = format!("({}) and {}", cond_str, extra);
                }
            }

            self.emit(&format!("    -- {} guard {}: {}", kind, i, cond_str));
            self.emit("    table.insert(transitions, {");
            self.indent += 1;
            self.emit(&format!("    guard = function(s) return {} end,", cond_str));
            self.emit("    effect = function(s)");
            self.indent += 1;
            for s in &guard.body {
                if matches!(s, Stmt::Expr(_, _) | Stmt::Skip(_)) {
                    continue;
                }
                match s {
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
                        if let Some(idx) = index {
                            let idx_str = self.expr_to_lua(idx);
                            self.emit(&format!("    s.{}[{} + 1] = {}", target_name, idx_str, v));
                        } else {
                            self.emit(&format!("    s.{} = {}", target_name, v));
                        }
                    }
                    Stmt::Break(_) => {
                        if let Some(ref pname) = self.current_proctype {
                            self.emit(&format!("    s._done_{} = true", pname));
                        } else {
                            self.emit("    -- break (no proctype context)");
                        }
                    }
                    Stmt::Goto(label, _) => {
                        self.emit(&format!("    -- goto {}", label));
                    }
                    Stmt::Assert(expr, _) => {
                        let e = self.expr_to_lua(expr);
                        self.emit(&format!("    assert({}, 'assertion failed')", e));
                    }
                    Stmt::Send {
                        channel, target, ..
                    } => {
                        let id = match target {
                            SendTarget::Ident(id) => id.clone(),
                            SendTarget::Value(_) => "expr".to_string(),
                            SendTarget::Sorted(val) => self.expr_to_lua(val),
                        };
                        self.emit(&format!(
                            "    chan_send({}, {}, ...)",
                            self.channel_to_lua(channel),
                            id
                        ));
                    }
                    Stmt::Recv {
                        channel, target, ..
                    } => {
                        if let RecvTarget::VarList(vars) = target {
                            let vs = vars.join(", ");
                            self.emit(&format!(
                                "    {}, _ = chan_recv({})",
                                vs,
                                self.channel_to_lua(channel)
                            ));
                        }
                    }
                    _ => {
                        self.emit("    -- (stmt not yet inlined in codegen)");
                    }
                }
            }
            self.indent -= 1;
            self.emit("    end,");
            {
                let body_refs: Vec<&Stmt> = guard.body.iter().collect();
                let deps = self.deps_of(guard.condition.as_ref(), &body_refs, &[]);
                self.emit_deps(&deps);
            }
            self.emit(&format!("    label = \"{}:guard{}\",", kind, i));
            self.indent -= 1;
            self.emit("    })");
        }
    }
}
