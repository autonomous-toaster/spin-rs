use super::LuaGenerator;
use crate::parser::ast::*;

impl LuaGenerator {
    pub(crate) fn emit_effect_for_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Assignment { target, value, .. } => self.emit_assignment_effect(target, value),
            Stmt::Skip(_) => self.emit_skip_effect(),
            Stmt::Break(_) => self.emit_break_effect(),
            Stmt::Assert(expr, _) => self.emit_assert_effect(expr),
            Stmt::Goto(_, _) => self.emit_goto_effect(),
            Stmt::Expr(_, _) => self.emit_expr_effect(),
            Stmt::Printf(fmt, args, _) => self.emit_printf_effect(fmt, args),
            Stmt::Run(name, args, _) => self.emit_run_effect(name, args),
            Stmt::Send { channel, args, .. } => self.emit_send_effect(channel, args),
            Stmt::Recv {
                channel, target, ..
            } => self.emit_recv_effect(channel, target),
            Stmt::Atomic(body, _) | Stmt::DStep(body, _) => self.emit_atomic_effect(body),
            Stmt::Unless { body, .. } => self.emit_unless_effect(body),
            Stmt::VarDecl(_) | Stmt::VarDecls(_) | Stmt::Label(_, _) => self.emit_decl_effect(),
            Stmt::For { .. } => self.emit_for_effect(),
            Stmt::If(guards) => self.emit_if_effect(guards),
            Stmt::Do(guards) => self.emit_do_effect(guards),
        }
    }

    pub(crate) fn emit_assignment_effect(&mut self, target: &str, value: &Expression) {
        let expr_str = self.expr_to_lua(value);
        let target_name = if self.global_vars.contains(target) {
            target.to_string()
        } else if let Some(ref pname) = self.current_proctype {
            format!("{}_{}", pname, target)
        } else {
            target.to_string()
        };
        self.emit(&format!("s.{} = {}", target_name, expr_str));
    }

    pub(crate) fn emit_skip_effect(&mut self) {
        self.emit("-- skip");
    }

    pub(crate) fn emit_break_effect(&mut self) {
        if let Some(ref pname) = self.current_proctype {
            self.emit(&format!("s._done_{} = true", pname));
        } else {
            self.emit("-- break");
        }
    }

    pub(crate) fn emit_assert_effect(&mut self, expr: &Expression) {
        let e = self.expr_to_lua(expr);
        self.emit(&format!("_spin_assert({}, 'assertion failed')", e));
    }

    pub(crate) fn emit_goto_effect(&mut self) {
        self.emit("-- goto");
    }

    pub(crate) fn emit_expr_effect(&mut self) {
        self.emit("-- expr");
    }

    pub(crate) fn emit_printf_effect(&mut self, fmt: &str, args: &[Expression]) {
        let args_str: Vec<String> = args.iter().map(|a| self.expr_to_lua(a)).collect();
        let args_concat = args_str.join(", ");
        self.emit(&format!("_spin_printf('{}', {})", fmt, args_concat));
    }

    pub(crate) fn emit_run_effect(&mut self, name: &str, args: &[Expression]) {
        let args_str: Vec<String> = args.iter().map(|a| self.expr_to_lua(a)).collect();
        let args_concat = args_str.join(", ");
        self.emit(&format!("run({}, {})", name, args_concat));
    }

    pub(crate) fn emit_send_effect(&mut self, channel: &Expression, args: &[Expression]) {
        let chan_name = self.channel_to_lua(channel);
        let args_str: Vec<String> = args.iter().map(|a| self.expr_to_lua(a)).collect();
        let args_concat = args_str.join(", ");
        self.emit(&format!("_spin_chan_send({}, {})", chan_name, args_concat));
    }

    pub(crate) fn emit_recv_effect(&mut self, channel: &Expression, target: &RecvTarget) {
        let chan_name = self.channel_to_lua(channel);
        match target {
            RecvTarget::VarList(vars) => {
                let val = format!("_spin_chan_recv({})", chan_name);
                if let Some(first_var) = vars.first() {
                    self.emit(&format!("s.{} = {}", first_var, val));
                }
            }
            _ => {
                self.emit(&format!("_spin_chan_recv({})", chan_name));
            }
        }
    }

    pub(crate) fn emit_atomic_effect(&mut self, body: &[Stmt]) {
        for s in body {
            self.emit_effect_for_stmt(s);
        }
    }

    pub(crate) fn emit_unless_effect(&mut self, body: &[Stmt]) {
        for s in body {
            self.emit_effect_for_stmt(s);
        }
    }

    pub(crate) fn emit_decl_effect(&mut self) {
        self.emit("-- decl/label in d_step");
    }

    pub(crate) fn emit_for_effect(&mut self) {
        self.emit("-- for loop in d_step (deferred)");
    }

    pub(crate) fn emit_if_effect(&mut self, guards: &[Guard]) {
        if let Some(guard) = guards.first() {
            let cond_str = match &guard.condition {
                Some(e) => self.expr_to_lua(e),
                None => "true".to_string(),
            };
            self.emit(&format!("if {} then", cond_str));
            self.indent += 1;
            for s in &guard.body {
                self.emit_effect_for_stmt(s);
            }
            self.indent -= 1;
            self.emit("end");
        }
    }

    pub(crate) fn emit_do_effect(&mut self, guards: &[Guard]) {
        if let Some(guard) = guards.first() {
            let cond_str = match &guard.condition {
                Some(e) => self.expr_to_lua(e),
                None => "true".to_string(),
            };
            self.emit(&format!("if {} then", cond_str));
            self.indent += 1;
            for s in &guard.body {
                self.emit_effect_for_stmt(s);
            }
            self.indent -= 1;
            self.emit("end");
        }
    }

    pub(crate) fn emit_guards(&mut self, kind: &str, guards: &[Guard], _depth: usize) {
        // Detect multi-statement guard bodies — need step-counter for interleaving.
        // Single-statement guards can use the simple flat approach.
        let has_multi_stmt: bool = guards.iter().any(|g| {
            g.body
                .iter()
                .filter(|s| !matches!(s, Stmt::Expr(_, _) | Stmt::Skip(_)))
                .count()
                > 1
        });

        if !has_multi_stmt {
            return self.emit_guards_flat(kind, guards);
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

            // Merge blocking exprs from this guard's body into condition
            let blocking: Vec<String> = guard
                .body
                .iter()
                .filter_map(|s| match s {
                    Stmt::Expr(e, _) => Some(self.expr_to_lua(e)),
                    _ => None,
                })
                .collect();
            let mut sel_cond = gc.clone();
            if !blocking.is_empty() {
                let extra = if blocking.len() == 1 {
                    blocking[0].clone()
                } else {
                    format!("({})", blocking.join(" and "))
                };
                if sel_cond == "true" {
                    sel_cond = extra;
                } else {
                    sel_cond = format!("({}) and {}", sel_cond, extra);
                }
            }

            let body_non_expr: Vec<&Stmt> = guard
                .body
                .iter()
                .filter(|s| !matches!(s, Stmt::Expr(_, _) | Stmt::Skip(_)))
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
            self.emit(&format!("    label = \"{}:sel{}\",", kind, gi));
            self.indent -= 1;
            self.emit("    })");
        }

        // Steps 1..N: emit one transition per body statement
        let total = body_steps.len();
        for (idx, (step, s)) in body_steps.iter().enumerate() {
            let next = if idx + 1 >= total { 0 } else { step + 1 };

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
                        "    effect = function(s) {} = {}; s.{} = {} end,",
                        target_expr, v, step_var, next
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
                    self.emit(&format!(
                        "    effect = function(s) s.{} = {} end,",
                        step_var, next
                    ));
                }
                Stmt::Assert(expr, _) => {
                    let e = self.expr_to_lua(expr);
                    self.emit(&format!(
                        "    guard = function(s) return s.{} == {} and ({}) end,",
                        step_var, step, e
                    ));
                    self.emit(&format!(
                        "    effect = function(s) s.{} = {} end,",
                        step_var, next
                    ));
                }
                Stmt::Send {
                    channel, target, ..
                } => {
                    let id = match target {
                        SendTarget::Ident(id) => id.clone(),
                        SendTarget::Value(_) => "expr".to_string(),
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
                    self.emit(&format!(
                        "    effect = function(s) s.{} = {} end,",
                        step_var, next
                    ));
                }
            }
            self.emit(&format!("    label = \"{}:step{}\",", kind, step));
            self.indent -= 1;
            self.emit("    })");
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
            self.emit(&format!("    label = \"{}:guard{}\",", kind, i));
            self.indent -= 1;
            self.emit("    })");
        }
    }
}
