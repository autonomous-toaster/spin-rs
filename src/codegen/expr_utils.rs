use super::GeneratedLua;
use super::LuaGenerator;
use crate::parser::ast::*;

impl LuaGenerator {
    pub(crate) fn emit_trailer(&mut self) {
        self.emit("-- Main transition dispatcher");
        self.emit("-- Called by Rust engine to enumerate transitions");
        self.emit("function _spin_get_transitions(state)");
        self.indent += 1;
        self.emit("    local all = {}");
        let names = self.proctype_names.clone();
        for name in names {
            self.emit(&format!("    local t = {}(state)", name));
            self.emit("    for _, tr in ipairs(t) do");
            self.emit("        table.insert(all, tr)");
            self.emit("    end");
        }
        self.emit("    return all");
        self.indent -= 1;
        self.emit("end");
        self.emit("");

        self.emit("-- State hashing callback");
        self.emit("function _spin_hash(state)");
        self.indent += 1;
        self.emit("    return state_hash(state)");
        self.indent -= 1;
        self.emit("end");
    }

    pub(crate) fn expr_to_lua(&self, expr: &Expression) -> String {
        self.special_expr_to_lua(expr)
    }

    pub(crate) fn unary_to_lua(&self, op: &UnaryOp, e: &Expression) -> String {
        use UnaryOp::*;
        match op {
            Not => {
                // Promela: !a  →  Lua: (a == 0) and 1 or 0
                // Lua treats 0 as truthy, so we must normalize to 0/1
                let e_str = self.expr_to_lua(e);
                format!("({} == 0) and 1 or 0", e_str)
            }
            _ => {
                let table: &[(UnaryOp, &str)] = &[
                    (BitNot, "~"),
                    (Neg, "-"),
                    (Always, "[]"),
                    (Eventually, "<>"),
                    (Next, "X"),
                ];
                let op_str = table
                    .iter()
                    .find(|(k, _)| k == op)
                    .map(|(_, v)| *v)
                    .unwrap_or("");
                format!("{}{}", op_str, self.expr_to_lua(e))
            }
        }
    }

    pub(crate) fn binary_op_to_lua(&self, op: &BinaryOp) -> &'static str {
        use BinaryOp::*;
        let table: &[(BinaryOp, &str)] = &[
            (Add, " + "),
            (Sub, " - "),
            (Mul, " * "),
            (Div, " / "),
            (Mod, " % "),
            (Eq, " == "),
            (Neq, " ~= "),
            (Lt, " < "),
            (Le, " <= "),
            (Gt, " > "),
            (Ge, " >= "),
            (And, " and "),
            (Or, " or "),
            (BitAnd, " & "),
            (BitOr, " | "),
            (BitXor, " // "),
            (Shl, " << "),
            (Shr, " >> "),
            (Impl, " -> "),
            (BiImpl, " <-> "),
            (Until, " U "),
            (Release, " V "),
        ];
        table
            .iter()
            .find(|(k, _)| k == op)
            .map(|(_, v)| *v)
            .unwrap_or("")
    }

    pub(crate) fn special_expr_to_lua(&self, expr: &Expression) -> String {
        match expr {
            Expression::IntLit(n) => n.to_string(),
            Expression::StringLit(s) => format!("\"{}\"", s),
            Expression::BoolLit(b) => b.to_string(),
            Expression::Ident(name) => {
                // Built-in variables like _pid are not prefixed
                if name == "_pid" {
                    "s._pid".to_string()
                } else if self.global_vars.contains(name.as_str()) {
                    // Global variables are stored unprefixed in the state
                    format!("s.{}", name)
                } else if let Some(ref pname) = self.current_proctype {
                    // Prefix local variables with current proctype name
                    format!("s.{}_{}", pname, name)
                } else {
                    format!("s.{}", name)
                }
            }
            Expression::ArrayAccess { name, index } => {
                // Promela arrays are 0-indexed, Lua tables are 1-indexed
                // Add 1 to the index: flag[i] -> s.flag[i + 1]
                let idx_str = self.expr_to_lua(index);
                if self.global_vars.contains(name.as_str()) {
                    format!("s.{}[{} + 1]", name, idx_str)
                } else if let Some(ref pname) = self.current_proctype {
                    format!("s.{}_{}[{} + 1]", pname, name, idx_str)
                } else {
                    format!("s.{}[{} + 1]", name, idx_str)
                }
            }
            Expression::UnaryOp { op, expr: e } => self.unary_to_lua(op, e),
            Expression::BinaryOp { op, left, right } => {
                use BinaryOp::*;
                match op {
                    And => {
                        // Promela: a && b  →  Lua: ((a ~= 0) and (b ~= 0)) and 1 or 0
                        let l = self.expr_to_lua(left);
                        let r = self.expr_to_lua(right);
                        format!("(({} ~= 0) and ({} ~= 0)) and 1 or 0", l, r)
                    }
                    Or => {
                        // Promela: a || b  →  Lua: ((a ~= 0) or (b ~= 0)) and 1 or 0
                        let l = self.expr_to_lua(left);
                        let r = self.expr_to_lua(right);
                        format!("(({} ~= 0) or ({} ~= 0)) and 1 or 0", l, r)
                    }
                    _ => format!(
                        "{}{}{}",
                        self.expr_to_lua(left),
                        self.binary_op_to_lua(op),
                        self.expr_to_lua(right)
                    ),
                }
            }
            Expression::FuncCall { name, args } => {
                let args_str: Vec<String> = args.iter().map(|a| self.expr_to_lua(a)).collect();
                format!("{}({})", name, args_str.join(", "))
            }
            Expression::Len(ch) => format!("chan_len('{}')", ch),
            Expression::Full(ch) => format!("chan_full('{}')", ch),
            Expression::Empty(ch) => format!("chan_empty('{}')", ch),
            Expression::NFull(ch) => format!("not chan_full('{}')", ch),
            Expression::NEmpty(ch) => format!("not chan_empty('{}')", ch),
            Expression::Enabled(pid) => format!("_spin_enabled({})", pid),
            Expression::Timeout => "_spin_timeout()".to_string(),
            Expression::NP_ => "_spin_np_()".to_string(),
            Expression::PcValue(pid) => format!("_spin_pc_value({})", self.expr_to_lua(pid)),
            Expression::Eval(expr) => format!("_spin_eval({})", self.expr_to_lua(expr)),
            Expression::GetPriority(pid) => {
                format!("_spin_get_priority({})", self.expr_to_lua(pid))
            }
            Expression::SetPriority { pid, value } => format!(
                "_spin_set_priority({}, {})",
                self.expr_to_lua(pid),
                self.expr_to_lua(value)
            ),
            Expression::RemoteRef { pid, name } => {
                let pid_str = self.expr_to_lua(pid);
                format!("_spin_remote_ref({}, '{}')", pid_str, name)
            }
            Expression::RecordAccess { record, field } => {
                format!("{}[\"{}\"]", self.expr_to_lua(record), field)
            }
            Expression::ChannelSend { channel, target } => {
                format!("s.{}[{}]", channel, self.expr_to_lua(target))
            }
            Expression::ChannelPoll { channel, .. } => format!("not chan_empty(s.{})", channel),
        }
    }

    pub(crate) fn finish(self) -> GeneratedLua {
        GeneratedLua {
            source: self.source,
            proctype_fn_names: self.proctype_names,
        }
    }
}
