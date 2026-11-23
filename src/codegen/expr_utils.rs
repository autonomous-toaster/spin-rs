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
            // Integer division and remainder follow C, not Lua: see `_spin_div`.
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
            // Promela has no boolean type: `true` and `false` are the integers 1 and 0. Emitting
            // Lua booleans put one in the state, where every later read inherited it - `!flag` and
            // `flag == 0` both came out wrong, in the direction of refuting a model pan proves -
            // and assigning one raised "error converting Lua boolean to i64" at the truncation
            // boundary.
            Expression::BoolLit(b) => {
                if *b {
                    "1".to_string()
                } else {
                    "0".to_string()
                }
            }
            Expression::Ident(name) => {
                // `_pid` is the id of the process *whose transition this is*, so it is a constant
                // of the emitted function rather than a state field. It used to be stored in the
                // state and set by each transition function while enumerating, which meant the
                // table was written by the last process enumerated and every guard and effect in
                // it saw that process's pid: Peterson's two instances both behaved as pid 1, and
                // the mutual exclusion assertion was refuted.
                if name == "_pid" {
                    // In a transition this is the id of the process making it; in the state layout
                    // there is no such process, and `_pid` is the instance being laid out.
                    match self.pid_override {
                        Some(pid) => pid.to_string(),
                        None => "_spin_pid".to_string(),
                    }
                } else if self.global_vars.contains(name.as_str()) {
                    // Global variables are stored unprefixed in the state.
                    format!("{}.{}", self.state_var, name)
                } else if let Some(ref pname) = self.current_proctype {
                    // Prefix local variables with current proctype name
                    format!("s.{}_{}", pname, name)
                } else {
                    format!("{}.{}", self.state_var, name)
                }
            }
            Expression::ArrayAccess { name, index } => {
                // Promela arrays are 0-indexed, Lua tables are 1-indexed
                // Add 1 to the index: flag[i] -> s.flag[i + 1]
                let idx_str = self.expr_to_lua(index);
                if self.global_vars.contains(name.as_str()) {
                    format!("{}.{}[{} + 1]", self.state_var, name, idx_str)
                } else if let Some(ref pname) = self.current_proctype {
                    format!("{}.{}_{}[{} + 1]", self.state_var, pname, name, idx_str)
                } else {
                    format!("{}.{}[{} + 1]", self.state_var, name, idx_str)
                }
            }
            Expression::UnaryOp { op, expr: e } => self.unary_to_lua(op, e),
            Expression::BinaryOp { op, left, right } => {
                use BinaryOp::*;
                match op {
                    // Promela's `&&` and `||` are value operators that read their operands
                    // with Promela truthiness, so each side goes through `_spin_truthy`. It used
                    // to be `((a ~= 0) and (b ~= 0))` with `a` spliced in unparenthesised, which
                    // Lua reads as `a == (0 ~= 0)` - a comparison against `false`. In Peterson's
                    // `(flag[1 - _pid] == 0 || turn == 1 - _pid)` every side became a comparison
                    // with `false`, so the guard was permanently disabled and mutual exclusion was
                    // "proved" by never entering the critical section.
                    And => {
                        let l = self.expr_to_lua(left);
                        let r = self.expr_to_lua(right);
                        format!("(_spin_truthy({}) and _spin_truthy({})) and 1 or 0", l, r)
                    }
                    Or => {
                        let l = self.expr_to_lua(left);
                        let r = self.expr_to_lua(right);
                        format!("(_spin_truthy({}) or _spin_truthy({})) and 1 or 0", l, r)
                    }
                    // Promela divides and takes remainders as C does, and Lua does neither:
                    // `/` is floating point and `%` floors, so `-7 % 2` was 1 where pan has -1.
                    Div => format!(
                        "_spin_div({}, {})",
                        self.expr_to_lua(left),
                        self.expr_to_lua(right)
                    ),
                    Mod => format!(
                        "_spin_mod({}, {})",
                        self.expr_to_lua(left),
                        self.expr_to_lua(right)
                    ),
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
            // The queue lives in the state, so every channel query takes it.
            Expression::Len(ch) => format!("chan_len(s, '{}')", ch),
            Expression::Full(ch) => format!("chan_full(s, '{}')", ch),
            Expression::Empty(ch) => format!("chan_empty(s, '{}')", ch),
            Expression::NFull(ch) => format!("not chan_full(s, '{}')", ch),
            Expression::NEmpty(ch) => format!("not chan_empty(s, '{}')", ch),
            Expression::Enabled(pid) => format!("_spin_enabled({})", pid),
            Expression::Timeout => "_spin_timeout()".to_string(),
            Expression::NP_ => "_spin_np_()".to_string(),
            // pan's `pc_value` reports pan's own statement numbering, so a model that branches
            // on it cannot agree with us by construction. Answering 0 was worse than refusing:
            // the field it read (`_pc_<number>`) was never the one we wrote (`_pc_<proctype>`),
            // so every model that asked got a confident zero.
            Expression::PcValue(_) => "_spin_pc_value_unsupported()".to_string(),
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
