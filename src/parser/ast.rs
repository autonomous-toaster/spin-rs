//! Promela IR (AST) data structures representing a parsed Promela model.

use std::fmt;

/// A parsed Promela model: the top-level container.
#[derive(Debug, Clone)]
pub struct PromelaModel {
    pub declarations: Vec<TopLevel>,
    pub source: Option<String>,
}

/// Top-level items in a Promela model.
#[derive(Debug, Clone)]
pub enum TopLevel {
    Proctype(ProctypeDef),
    Init(InitDef),
    NeverClaim(NeverClaim),
    Ltl(LtlFormula),
    GlobalVar(VarDecl),
    ChanDecl {
        name: String,
        capacity: i64,
        line: usize,
    },
    PreprocessorDirective(String),
    /// Embedded C code (via Lua FFI in spin-rs)
    CCode(String, usize),
    /// Embedded C state (variable declarations via Lua)
    CState(Vec<VarDecl>, usize),
    /// Inline macro definition (expansion deferred)
    Inline(InlineDef),
    /// Channel array declaration: chan name[N];
    ChannelArray {
        name: String,
        size: i64,
        line: usize,
    },
}

/// A proctype definition (process template).
#[derive(Debug, Clone)]
pub struct ProctypeDef {
    pub name: String,
    pub active: bool,
    pub provided: Option<Expression>,
    pub parameters: Vec<VarDecl>,
    pub body: Vec<Stmt>,
    pub pid: Option<i64>,
    pub line: usize,
}

/// The `init` block.
#[derive(Debug, Clone)]
pub struct InitDef {
    pub body: Vec<Stmt>,
    pub line: usize,
}

/// An inline macro definition.
#[derive(Debug, Clone)]
pub struct InlineDef {
    pub name: String,
    pub parameters: Vec<String>,
    pub body: Vec<Stmt>,
    pub line: usize,
}

/// A `never` claim for liveness property checking.
#[derive(Debug, Clone)]
pub struct NeverClaim {
    pub body: Vec<Stmt>,
    pub line: usize,
}

/// An inline LTL formula.
#[derive(Debug, Clone)]
pub struct LtlFormula {
    pub name: Option<String>,
    pub formula: String,
    pub line: usize,
}

/// Variable type.
#[derive(Debug, Clone, PartialEq)]
pub enum VarType {
    Bit,
    Bool,
    Byte,
    Short,
    Int,
    Unsigned(Option<u32>),
    Chan,
    Mtype,
    Named(String),
}

/// Variable declaration.
#[derive(Debug, Clone)]
pub struct VarDecl {
    pub var_type: VarType,
    pub name: String,
    pub array_size: Option<i64>,
    pub init: Option<Box<Expression>>,
    pub line: usize,
}

/// Statements.
#[derive(Debug, Clone)]
pub enum Stmt {
    VarDecl(VarDecl),
    VarDecls(Vec<VarDecl>),
    Assignment {
        target: String,
        index: Option<Box<Expression>>,
        value: Box<Expression>,
        line: usize,
    },
    If(Vec<Guard>),
    Do(Vec<Guard>),
    Goto(String, usize),
    Break(usize),
    Assert(Expression, usize),
    Printf(String, Vec<Expression>, usize),
    Expr(Expression, usize),
    Atomic(Vec<Stmt>, usize),
    DStep(Vec<Stmt>, usize),
    Send {
        channel: Box<Expression>,
        target: SendTarget,
        args: Vec<Expression>,
        line: usize,
    },
    Recv {
        channel: Box<Expression>,
        target: RecvTarget,
        line: usize,
    },
    Skip(usize),
    Unless {
        body: Vec<Stmt>,
        handler: Vec<Stmt>,
        line: usize,
    },
    Run(String, Vec<Expression>, usize),
    Label(String, usize),
    For {
        init: Box<Stmt>,
        condition: Expression,
        update: Box<Stmt>,
        body: Vec<Stmt>,
        line: usize,
    },
}

/// A guard in if/fi or do/od.
#[derive(Debug, Clone)]
pub struct Guard {
    pub condition: Option<Expression>,
    pub body: Vec<Stmt>,
    pub line: usize,
}

/// Target of a channel send operation (`!`).
#[derive(Debug, Clone)]
pub enum SendTarget {
    Value(Expression),
    Ident(String),
}

/// Target of a channel receive operation (`?`).
#[derive(Debug, Clone)]
pub enum RecvTarget {
    VarList(Vec<String>),
    Eval(Expression),
    Poll(Expression),
}

/// Expressions.
#[derive(Debug, Clone)]
pub enum Expression {
    IntLit(i64),
    StringLit(String),
    BoolLit(bool),
    Ident(String),
    ArrayAccess {
        name: String,
        index: Box<Expression>,
    },
    RecordAccess {
        record: Box<Expression>,
        field: String,
    },
    UnaryOp {
        op: UnaryOp,
        expr: Box<Expression>,
    },
    BinaryOp {
        op: BinaryOp,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    FuncCall {
        name: String,
        args: Vec<Expression>,
    },
    ChannelSend {
        channel: String,
        target: Box<Expression>,
    },
    ChannelPoll {
        channel: String,
        condition: Box<Expression>,
    },
    Len(String),
    Full(String),
    Empty(String),
    NFull(String),
    NEmpty(String),
    Enabled(String),
    Timeout,
    RemoteRef {
        pid: Box<Expression>,
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Not,
    BitNot,
    Neg,
    Next,       // X (LTL)
    Always,     // [] (LTL)
    Eventually, // <> (LTL)
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Neq,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Impl,    // -> (LTL)
    BiImpl,  // <-> (LTL)
    Until,   // U (LTL)
    Release, // V (LTL)
}

fn write_vartype(f: &mut fmt::Formatter<'_>, vt: &VarType) -> fmt::Result {
    use VarType::*;
    let simple_table: &[(VarType, &str)] = &[
        (Bit, "bit"),
        (Bool, "bool"),
        (Byte, "byte"),
        (Short, "short"),
        (Int, "int"),
        (Chan, "chan"),
        (Mtype, "mtype"),
    ];
    if let Some((_, name)) = simple_table
        .iter()
        .find(|(k, _)| std::mem::discriminant(k) == std::mem::discriminant(vt))
    {
        return write!(f, "{}", name);
    }
    match vt {
        Unsigned(w) => match w {
            Some(n) => write!(f, "unsigned({})", n),
            None => write!(f, "unsigned"),
        },
        Named(n) => write!(f, "{}", n),
        _ => Ok(()),
    }
}

impl fmt::Display for VarType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_vartype(f, self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vartype_display_bit() {
        assert_eq!(VarType::Bit.to_string(), "bit");
    }

    #[test]
    fn test_vartype_display_bool() {
        assert_eq!(VarType::Bool.to_string(), "bool");
    }

    #[test]
    fn test_vartype_display_byte() {
        assert_eq!(VarType::Byte.to_string(), "byte");
    }

    #[test]
    fn test_vartype_display_short() {
        assert_eq!(VarType::Short.to_string(), "short");
    }

    #[test]
    fn test_vartype_display_int() {
        assert_eq!(VarType::Int.to_string(), "int");
    }

    #[test]
    fn test_vartype_display_chan() {
        assert_eq!(VarType::Chan.to_string(), "chan");
    }

    #[test]
    fn test_vartype_display_mtype() {
        assert_eq!(VarType::Mtype.to_string(), "mtype");
    }

    #[test]
    fn test_vartype_display_unsigned_with_width() {
        assert_eq!(VarType::Unsigned(Some(8)).to_string(), "unsigned(8)");
    }

    #[test]
    fn test_vartype_display_unsigned_without_width() {
        assert_eq!(VarType::Unsigned(None).to_string(), "unsigned");
    }

    #[test]
    fn test_vartype_display_named() {
        assert_eq!(VarType::Named("mytype".to_string()).to_string(), "mytype");
    }
}
