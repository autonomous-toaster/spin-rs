use super::LuaGenerator;
use crate::parser::ast::*;

impl LuaGenerator {
    pub(crate) fn emit_assignment(
        &mut self,
        target: &str,
        field: Option<&str>,
        value: &Expression,
        depth: usize,
    ) {
        let expr_str = self.expr_to_lua(value);
        self.emit(&format!(
            "    -- T{}: {}{}",
            depth,
            target,
            field.map(|f| format!(".{}", f)).unwrap_or_default()
        ));
        self.emit("    table.insert(transitions, {");
        self.indent += 1;
        self.emit("    guard = function() return true end,");
        // Prefix local variables with current proctype name
        let target_name = if let Some(ref pname) = self.current_proctype {
            format!("{}_{}", pname, target)
        } else {
            target.to_string()
        };
        if let Some(f) = field {
            self.emit(&format!(
                "    effect = function(s) s.{}[\"{}\"] = {} end",
                target_name, f, expr_str
            ));
        } else {
            self.emit(&format!(
                "    effect = function(s) s.{} = {} end",
                target_name, expr_str
            ));
        }
        self.indent -= 1;
        self.emit("    })");
    }

    pub(crate) fn emit_assert_stmt(&mut self, expr: &Expression) {
        let e = self.expr_to_lua(expr);
        self.emit(&format!("    -- assert({})", e));
        self.emit("    table.insert(transitions, {");
        self.indent += 1;
        // Assert condition is the guard: if condition is true, assertion passes (enabled);
        // if false, the transition is not enabled, causing deadlock -> violation detected.
        self.emit(&format!("    guard = function(s) return {} end,", e));
        self.emit("    effect = function(s) end");
        self.indent -= 1;
        self.emit("    })");
    }

    pub(crate) fn channel_to_lua(&self, channel: &Expression) -> String {
        // Returns a Lua expression string for the channel name, including quotes.
        // For ident: 'tok' (quoted string literal)
        // For indexed: 'tok_' .. tostring(i) (runtime concatenation)
        match channel {
            Expression::Ident(name) => format!("'{}'", name),
            Expression::ArrayAccess { name, index } => {
                let idx_str = self.expr_to_lua(index);
                format!("'{}_' .. tostring({})", name, idx_str)
            }
            _ => format!("'{}'", self.expr_to_lua(channel)),
        }
    }

    pub(crate) fn emit_send_stmt(
        &mut self,
        channel: &Expression,
        target: &SendTarget,
        args: &[Expression],
    ) {
        let chan_name = self.channel_to_lua(channel);
        let args_str: Vec<String> = args.iter().map(|a| self.expr_to_lua(a)).collect();
        let args_concat = args_str.join(", ");
        match target {
            SendTarget::Ident(id) => {
                self.emit("    table.insert(transitions, {");
                self.indent += 1;
                self.emit(&format!(
                    "    guard = function(s) return not chan_full({}) end,",
                    chan_name
                ));
                let send_args = if args_concat.is_empty() {
                    id.clone()
                } else {
                    format!("{}, {}", id, args_concat)
                };
                self.emit(&format!(
                    "    effect = function(s) _spin_chan_send(s, {}, {}) end",
                    chan_name, send_args
                ));
                self.indent -= 1;
                self.emit("    })");
            }
            SendTarget::Value(val) => {
                let val_str = self.expr_to_lua(val);
                self.emit("    table.insert(transitions, {");
                self.indent += 1;
                self.emit(&format!(
                    "    guard = function(s) return not chan_full({}) end,",
                    chan_name
                ));
                let send_args = if args_concat.is_empty() {
                    val_str
                } else {
                    format!("{}, {}", val_str, args_concat)
                };
                self.emit(&format!(
                    "    effect = function(s) _spin_chan_send(s, {}, {}) end",
                    chan_name, send_args
                ));
                self.indent -= 1;
                self.emit("    })");
            }
            SendTarget::Sorted(val) => {
                let val_str = self.expr_to_lua(val);
                self.emit("    table.insert(transitions, {");
                self.indent += 1;
                self.emit(&format!(
                    "    guard = function(s) return not chan_full({}) end,",
                    chan_name
                ));
                self.emit(&format!(
                    "    effect = function(s) _spin_chan_send_sorted(s, {}, {}) end",
                    chan_name, val_str
                ));
                self.indent -= 1;
                self.emit("    })");
            }
        }
    }

    pub(crate) fn emit_recv_stmt(&mut self, channel: &Expression, target: &RecvTarget) {
        let chan_name = self.channel_to_lua(channel);
        match target {
            RecvTarget::VarList(vars) => {
                let vars_str = vars.join(", ");
                self.emit("    table.insert(transitions, {");
                self.indent += 1;
                self.emit(&format!(
                    "    guard = function(s) return not chan_empty({}) end,",
                    chan_name
                ));
                self.emit(&format!(
                    "    effect = function(s) local _v = _spin_chan_recv(s, {}); {} = _v end",
                    chan_name, vars_str
                ));
                self.indent -= 1;
                self.emit("    })");
            }
            RecvTarget::Eval(expr) => {
                let val_str = self.expr_to_lua(expr);
                self.emit("    table.insert(transitions, {");
                self.indent += 1;
                self.emit(&format!(
                    "    guard = function(s) return _spin_chan_poll({}, {}) ~= nil end,",
                    chan_name, val_str
                ));
                self.emit(&format!(
                    "    effect = function(s) _spin_chan_recv_eval(s, {}, {}) end",
                    chan_name, val_str
                ));
                self.indent -= 1;
                self.emit("    })");
            }
            RecvTarget::Poll(expr) => {
                let val_str = self.expr_to_lua(expr);
                self.emit("    table.insert(transitions, {");
                self.indent += 1;
                self.emit(&format!(
                    "    guard = function(s) return _spin_chan_poll({}, {}) ~= nil end,",
                    chan_name, val_str
                ));
                self.emit("    effect = function(s) end");
                self.indent -= 1;
                self.emit("    })");
            }
            RecvTarget::Random(vars) => {
                let vars_str = vars.join(", ");
                self.emit("    table.insert(transitions, {");
                self.indent += 1;
                self.emit(&format!(
                    "    guard = function(s) return not chan_empty({}) end,",
                    chan_name
                ));
                self.emit(&format!(
                    "    effect = function(s) local _v = _spin_chan_recv_random(s, {}); {} = _v end",
                    chan_name, vars_str
                ));
                self.indent -= 1;
                self.emit("    })");
            }
        }
    }

    pub(crate) fn emit_stmts(&mut self, stmts: &[Stmt], depth: usize) {
        for stmt in stmts {
            match stmt {
                Stmt::Assignment {
                    target,
                    field,
                    value,
                    ..
                } => {
                    self.emit_assignment(target, field.as_deref(), value, depth);
                }
                Stmt::If(guards) => {
                    self.emit_guards("if", guards, depth);
                }
                Stmt::Do(guards) => {
                    self.emit_guards("do", guards, depth);
                }
                Stmt::Goto(label, _) => {
                    self.emit_goto_stmt(label);
                }
                Stmt::Break(_) => {
                    self.emit_break_stmt();
                }
                Stmt::Assert(expr, _) => {
                    self.emit_assert_stmt(expr);
                }
                Stmt::Printf(fmt, args, _) => {
                    let args_str: Vec<String> = args.iter().map(|a| self.expr_to_lua(a)).collect();
                    let args_concat = args_str.join(", ");
                    self.emit(&format!("    printf('{}', {})", fmt, args_concat));
                }
                Stmt::Send {
                    channel,
                    target,
                    args,
                    ..
                } => {
                    self.emit_send_stmt(channel, target, args);
                }
                Stmt::Recv {
                    channel, target, ..
                } => {
                    self.emit_recv_stmt(channel, target);
                }
                Stmt::Run(name, args, _) => {
                    let args_str: Vec<String> = args.iter().map(|a| self.expr_to_lua(a)).collect();
                    let args_concat = args_str.join(", ");
                    self.emit(&format!("    run({}, {})", name, args_concat));
                }
                Stmt::Unless { body, handler, .. } => {
                    self.emit_unless_block(body, handler, depth);
                }
                Stmt::Skip(_) => {
                    self.emit("    -- skip");
                }
                Stmt::Atomic(body, _) | Stmt::DStep(body, _) => {
                    let is_dstep = matches!(stmt, Stmt::DStep(_, _));
                    self.emit_atomic_block(body, is_dstep, depth);
                }
                Stmt::Expr(e, _) => {
                    // Check if this is an inline function call
                    if let Expression::FuncCall { name, args } = e {
                        if let Some(inline_def) = self.inlines.get(name.as_str()).cloned() {
                            self.emit(&format!("    -- inline {} expansion", name));
                            let subs: Vec<(String, String)> = inline_def
                                .parameters
                                .iter()
                                .zip(args.iter())
                                .map(|(p, a)| (p.clone(), self.expr_to_lua(a)))
                                .collect();
                            for s in &inline_def.body {
                                self.emit_inline_stmt_with_subs(s, &subs, depth);
                            }
                        } else {
                            self.emit("    -- expression statement");
                        }
                    } else {
                        self.emit("    -- expression statement");
                    }
                }
                Stmt::VarDecl(_) | Stmt::VarDecls(_) => {
                    // Variable declarations are handled in _spin_init_state
                    self.emit("    -- var decl (already initialized)");
                }
                Stmt::For {
                    init,
                    condition,
                    update,
                    body,
                    ..
                } => {
                    self.emit("    -- for loop (sequential expansion)");
                    self.emit("    -- init");
                    self.emit_stmts(std::slice::from_ref(init.as_ref()), depth);
                    self.emit(&format!("    -- while {} do", self.expr_to_lua(condition)));
                    self.indent += 1;
                    for s in body {
                        self.emit_stmts(std::slice::from_ref(s), depth + 1);
                    }
                    self.indent -= 1;
                    self.emit("    -- update");
                    self.emit_stmts(std::slice::from_ref(update.as_ref()), depth);
                    self.emit("    -- end");
                }
                Stmt::Label(name, _) => {
                    self.emit_label_stmt(name);
                }
            }
        }
    }

    /// Extract guard expression for a statement (for combined guards in d_step/atomic)
    pub(crate) fn guard_for_stmt(&self, stmt: &Stmt) -> String {
        match stmt {
            Stmt::Send { channel, .. } => {
                format!("not chan_full({})", self.channel_to_lua(channel))
            }
            Stmt::Recv { channel, .. } => {
                format!("not chan_empty({})", self.channel_to_lua(channel))
            }
            Stmt::Assert(expr, _) => self.expr_to_lua(expr),
            Stmt::Skip(_) | Stmt::Break(_) => "true".to_string(),
            Stmt::Goto(_, _) => "true".to_string(),
            Stmt::Assignment { .. } | Stmt::Expr(_, _) => "true".to_string(),
            Stmt::Printf(_, _, _) => "true".to_string(),
            Stmt::Run(_, _, _) => "true".to_string(),
            Stmt::Atomic(body, _) | Stmt::DStep(body, _) => {
                if body.is_empty() {
                    "true".to_string()
                } else {
                    let gs: Vec<String> = body.iter().map(|s| self.guard_for_stmt(s)).collect();
                    gs.join(" and ")
                }
            }
            Stmt::Unless { .. } => "true".to_string(),
            Stmt::VarDecl(_) | Stmt::Label(_, _) | Stmt::VarDecls(_) | Stmt::For { .. } => {
                "true".to_string()
            }
            Stmt::If(guards) => {
                if guards.is_empty() {
                    "true".to_string()
                } else {
                    let gs: Vec<String> = guards
                        .iter()
                        .map(|g| match &g.condition {
                            Some(e) => self.expr_to_lua(e),
                            None => "true".to_string(),
                        })
                        .collect();
                    gs.join(" or ")
                }
            }
            Stmt::Do(guards) => {
                if guards.is_empty() {
                    "false".to_string()
                } else {
                    let gs: Vec<String> = guards
                        .iter()
                        .map(|g| match &g.condition {
                            Some(e) => self.expr_to_lua(e),
                            None => "true".to_string(),
                        })
                        .collect();
                    gs.join(" or ")
                }
            }
        }
    }
}
