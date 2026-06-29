use super::LuaGenerator;
use crate::parser::ast::*;

impl LuaGenerator {
    pub(crate) fn emit_inline_stmt_with_subs(
        &mut self,
        stmt: &Stmt,
        subs: &[(String, String)],
        depth: usize,
    ) {
        match stmt {
            Stmt::Atomic(body, _) | Stmt::DStep(body, _) => {
                self.emit(&format!(
                    "    -- atomic/d_step from inline (depth {})",
                    depth
                ));
                for s in body {
                    self.emit_inline_stmt_with_subs(s, subs, depth + 1);
                }
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
                    self.emit(&format!(
                        "    table.insert(transitions, {{ guard = function(s) return true end, effect = function(s) s.{}[{} + 1] = {} end }})",
                        target_name, idx_str, v
                    ));
                } else {
                    self.emit(&format!(
                        "    table.insert(transitions, {{ guard = function(s) return true end, effect = function(s) s.{} = {} end }})",
                        target_name, v
                    ));
                }
            }
            Stmt::Skip(_) => {}
            Stmt::Expr(e, _) => {
                if let Expression::FuncCall { name, args } = e
                    && let Some(inline_def) = self.inlines.get(name.as_str()).cloned()
                {
                    let inner_subs: Vec<(String, String)> = inline_def
                        .parameters
                        .iter()
                        .zip(args.iter())
                        .map(|(p, a)| (p.clone(), self.substitute_expr(a, subs)))
                        .collect();
                    for s in &inline_def.body {
                        self.emit_inline_stmt_with_subs(s, &inner_subs, depth + 1);
                    }
                }
            }
            Stmt::If(guards) => {
                if let Some(guard) = guards.first() {
                    for s in &guard.body {
                        self.emit_inline_stmt_with_subs(s, subs, depth + 1);
                    }
                }
            }
            _ => {
                self.emit("    -- (inline stmt not expanded)");
            }
        }
    }

    pub(crate) fn substitute_expr(&self, expr: &Expression, subs: &[(String, String)]) -> String {
        match expr {
            Expression::Ident(name) => self.substitute_var(name, subs),
            Expression::IntLit(n) => n.to_string(),
            Expression::BoolLit(b) => b.to_string(),
            Expression::ArrayAccess { name, index } => {
                let idx = self.substitute_expr(index, subs);
                format!("s.{}[{} + 1]", self.substitute_var(name, subs), idx)
            }
            Expression::BinaryOp { op, left, right } => {
                let l = self.substitute_expr(left, subs);
                let r = self.substitute_expr(right, subs);
                format!("{}{}{}", l, self.binary_op_to_lua(op), r)
            }
            Expression::UnaryOp { op, expr: e } => {
                let e_str = self.substitute_expr(e, subs);
                format!("{}{}", self.unary_to_lua(op, e), e_str)
            }
            _ => self.expr_to_lua(expr),
        }
    }

    pub(crate) fn substitute_var(&self, name: &str, subs: &[(String, String)]) -> String {
        for (param, replacement) in subs {
            if name == param {
                return replacement.clone();
            }
        }
        name.to_string()
    }

    /// Pre-pass: collect all labels in a statement list and assign step numbers.
    pub(crate) fn collect_labels(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            match stmt {
                Stmt::Label(name, _) if !self.label_steps.contains_key(name) => {
                    let step = self.next_label_step;
                    self.next_label_step += 1;
                    self.label_steps.insert(name.clone(), step);
                }
                Stmt::If(guards) | Stmt::Do(guards) => {
                    for g in guards {
                        self.collect_labels(&g.body);
                    }
                }
                Stmt::Atomic(body, _) | Stmt::DStep(body, _) => {
                    self.collect_labels(body);
                }
                Stmt::Unless { body, handler, .. } => {
                    self.collect_labels(body);
                    self.collect_labels(handler);
                }
                Stmt::For { body, .. } => {
                    for s in body {
                        self.collect_labels(std::slice::from_ref(s));
                    }
                }
                _ => {}
            }
        }
    }

    /// Emit a goto transition: sets _pc to the target label's step number
    /// and marks the proctype as done.
    pub(crate) fn emit_goto_stmt(&mut self, label: &str) {
        let pc_var = if let Some(ref pname) = self.current_proctype {
            format!("_pc_{}", pname)
        } else {
            "_pc".to_string()
        };
        let done_var = if let Some(ref pname) = self.current_proctype {
            format!("_done_{}", pname)
        } else {
            "_done".to_string()
        };
        let target_step = self.label_steps.get(label).copied().unwrap_or(0);
        self.emit(&format!("    -- goto {}", label));
        self.emit("    table.insert(transitions, {");
        self.indent += 1;
        self.emit("    guard = function(s) return true end,");
        self.emit(&format!(
            "    effect = function(s) s.{} = true; s.{} = {} end,",
            done_var, pc_var, target_step
        ));
        self.emit(&format!("    label = \"goto:{}\",", label));
        self.indent -= 1;
        self.emit("    })");
    }

    /// Emit a break transition: sets _pc to the exit step of the enclosing do-loop
    /// and marks the proctype as done.
    pub(crate) fn emit_break_stmt(&mut self) {
        let pc_var = if let Some(ref pname) = self.current_proctype {
            format!("_pc_{}", pname)
        } else {
            "_pc".to_string()
        };
        let done_var = if let Some(ref pname) = self.current_proctype {
            format!("_done_{}", pname)
        } else {
            "_done".to_string()
        };
        let exit_step = self.do_exit_stack.last().copied().unwrap_or(0);
        self.emit("    -- break");
        self.emit("    table.insert(transitions, {");
        self.indent += 1;
        self.emit("    guard = function(s) return true end,");
        self.emit(&format!(
            "    effect = function(s) s.{} = true; s.{} = {} end,",
            done_var, pc_var, exit_step
        ));
        self.emit(&format!("    label = \"break:exit{}\",", exit_step));
        self.indent -= 1;
        self.emit("    })");
    }

    /// Emit a label transition: checks _pc == label_step, executes label body,
    /// and resets _pc to 0 and _done to false.
    pub(crate) fn emit_label_stmt(&mut self, name: &str) {
        let pc_var = if let Some(ref pname) = self.current_proctype {
            format!("_pc_{}", pname)
        } else {
            "_pc".to_string()
        };
        let done_var = if let Some(ref pname) = self.current_proctype {
            format!("_done_{}", pname)
        } else {
            "_done".to_string()
        };
        let progress_var = if let Some(ref pname) = self.current_proctype {
            format!("_progress_{}", pname)
        } else {
            "_progress".to_string()
        };
        let label_step = self.label_steps.get(name).copied().unwrap_or(0);
        let is_progress = name == "progress";
        self.emit(&format!(
            "    -- label {} (step {}){}",
            name,
            label_step,
            if is_progress { " [progress]" } else { "" }
        ));
        self.emit("    table.insert(transitions, {");
        self.indent += 1;
        self.emit(&format!(
            "    guard = function(s) return s.{} == {} end,",
            pc_var, label_step
        ));
        if is_progress {
            self.emit(&format!(
                "    effect = function(s) s.{} = false; s.{} = 0; s.{} = true end,",
                done_var, pc_var, progress_var
            ));
        } else {
            self.emit(&format!(
                "    effect = function(s) s.{} = false; s.{} = 0 end,",
                done_var, pc_var
            ));
        }
        self.emit(&format!("    label = \"label:{}\",", name));
        self.indent -= 1;
        self.emit("    })");
    }

    /// Emit a state machine for atomic/d_step block.
    pub(crate) fn emit_atomic_block(&mut self, body: &[Stmt], is_dstep: bool, _depth: usize) {
        if body.is_empty() {
            self.emit("    -- empty atomic/d_step");
            return;
        }
        let step_var = if let Some(ref pname) = self.current_proctype {
            format!("_atomic_step_{}", pname)
        } else {
            "_atomic_step".to_string()
        };
        let num_steps = body.len();
        self.emit(&format!(
            "    -- {} block with {} states",
            if is_dstep { "d_step" } else { "atomic" },
            num_steps
        ));
        for (i, stmt) in body.iter().enumerate() {
            let next_step = if i + 1 >= num_steps { 0 } else { i + 1 };
            let is_last = i + 1 >= num_steps;
            self.emit("    table.insert(transitions, {");
            self.indent += 1;
            let stmt_guard = self.guard_for_stmt(stmt);
            if i == 0 {
                if stmt_guard == "true" {
                    self.emit(&format!(
                        "    guard = function(s) return s.{} == 0 end,",
                        step_var
                    ));
                } else {
                    self.emit(&format!(
                        "    guard = function(s) return s.{} == 0 and ({}) end,",
                        step_var, stmt_guard
                    ));
                }
            } else {
                if stmt_guard == "true" {
                    self.emit(&format!(
                        "    guard = function(s) return s.{} == {} end,",
                        step_var, i
                    ));
                } else {
                    self.emit(&format!(
                        "    guard = function(s) return s.{} == {} and ({}) end,",
                        step_var, i, stmt_guard
                    ));
                }
            }
            self.emit("    effect = function(s)");
            self.indent += 1;
            self.emit_effect_for_stmt(stmt);
            if is_last {
                self.emit(&format!("    s.{} = 0", step_var));
            } else {
                self.emit(&format!("    s.{} = {}", step_var, next_step));
            }
            self.indent -= 1;
            self.emit("    end,");
            self.emit(&format!(
                "    label = \"{}:step{}\",",
                if is_dstep { "dstep" } else { "atomic" },
                i
            ));
            self.indent -= 1;
            self.emit("    })");
        }
        if is_dstep {
            self.emit("    -- d_step: intermediate states are transient (not stored)");
        }
    }

    /// Emit state machine for unless block.
    pub(crate) fn emit_unless_block(&mut self, body: &[Stmt], handler: &[Stmt], _depth: usize) {
        if body.is_empty() {
            self.emit("    -- empty unless body");
            return;
        }
        let step_var = if let Some(ref pname) = self.current_proctype {
            format!("_unless_step_{}", pname)
        } else {
            "_unless_step".to_string()
        };
        let num_steps = body.len();
        let handler_step = num_steps;
        self.emit(&format!(
            "    -- unless block: {} body steps + handler",
            num_steps
        ));
        for (i, stmt) in body.iter().enumerate() {
            let next_step = i + 1;
            let is_last = i + 1 >= num_steps;
            self.emit("    table.insert(transitions, {");
            self.indent += 1;
            let stmt_guard = self.guard_for_stmt(stmt);
            if stmt_guard == "true" {
                self.emit(&format!(
                    "    guard = function(s) return s.{} == {} end,",
                    step_var, i
                ));
            } else {
                self.emit(&format!(
                    "    guard = function(s) return s.{} == {} and ({}) end,",
                    step_var, i, stmt_guard
                ));
            }
            self.emit("    effect = function(s)");
            self.indent += 1;
            self.emit_effect_for_stmt(stmt);
            if is_last {
                self.emit(&format!("    s.{} = 0", step_var));
            } else {
                self.emit(&format!("    s.{} = {}", step_var, next_step));
            }
            self.indent -= 1;
            self.emit("    end,");
            self.emit(&format!("    label = \"unless:body{}\",", i));
            self.indent -= 1;
            self.emit("    })");
            if let Some(handler_first) = handler.first() {
                let handler_guard = self.guard_for_stmt(handler_first);
                if handler_guard != "false" {
                    self.emit("    table.insert(transitions, {");
                    self.indent += 1;
                    if handler_guard == "true" {
                        self.emit(&format!(
                            "    guard = function(s) return s.{} == {} end,",
                            step_var, i
                        ));
                    } else {
                        self.emit(&format!(
                            "    guard = function(s) return s.{} == {} and ({}) end,",
                            step_var, i, handler_guard
                        ));
                    }
                    self.emit(&format!(
                        "    effect = function(s) s.{} = {} end,",
                        step_var, handler_step
                    ));
                    self.emit(&format!("    label = \"unless:escape{}\",", i));
                    self.indent -= 1;
                    self.emit("    })");
                }
            }
        }
        for (i, stmt) in handler.iter().enumerate() {
            let current_step = handler_step + i;
            let next_step = if i + 1 >= handler.len() {
                handler_step + handler.len()
            } else {
                current_step + 1
            };
            self.emit("    table.insert(transitions, {");
            self.indent += 1;
            let stmt_guard = self.guard_for_stmt(stmt);
            if stmt_guard == "true" {
                self.emit(&format!(
                    "    guard = function(s) return s.{} == {} end,",
                    step_var, current_step
                ));
            } else {
                self.emit(&format!(
                    "    guard = function(s) return s.{} == {} and ({}) end,",
                    step_var, current_step, stmt_guard
                ));
            }
            self.emit("    effect = function(s)");
            self.indent += 1;
            self.emit_effect_for_stmt(stmt);
            self.emit(&format!("    s.{} = {}", step_var, next_step));
            self.indent -= 1;
            self.emit("    end,");
            self.emit(&format!("    label = \"unless:handler{}\",", i));
            self.indent -= 1;
            self.emit("    })");
        }
    }
}
