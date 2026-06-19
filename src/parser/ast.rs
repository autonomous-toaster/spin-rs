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
}

/// A proctype definition (process template).
#[derive(Debug, Clone)]
pub struct ProctypeDef {
    pub name: String,
    pub active: bool,
    pub provided: Option<Expression>,
    pub parameters: Vec<VarDecl>,
    pub body: Vec<Stmt>,
    pub line: usize,
}

/// The `init` block.
#[derive(Debug, Clone)]
pub struct InitDef {
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
        channel: String,
        target: SendTarget,
        args: Vec<Expression>,
        line: usize,
    },
    Recv {
        channel: String,
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

#[derive(Debug, Clone)]
pub enum UnaryOp {
    Not,
    BitNot,
    Neg,
    Next,       // X (LTL)
    Always,     // [] (LTL)
    Eventually, // <> (LTL)
}

#[derive(Debug, Clone)]
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

impl fmt::Display for VarType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VarType::Bit => write!(f, "bit"),
            VarType::Bool => write!(f, "bool"),
            VarType::Byte => write!(f, "byte"),
            VarType::Short => write!(f, "short"),
            VarType::Int => write!(f, "int"),
            VarType::Unsigned(None) => write!(f, "unsigned"),
            VarType::Unsigned(Some(w)) => write!(f, "unsigned({})", w),
            VarType::Chan => write!(f, "chan"),
            VarType::Mtype => write!(f, "mtype"),
            VarType::Named(n) => write!(f, "{}", n),
        }
    }
}
