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
}
