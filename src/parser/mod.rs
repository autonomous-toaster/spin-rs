//! Promela parser: converts Promela source text into an AST using nom.

pub mod ast;

use nom::{
    IResult,
    branch::alt,
    bytes::complete::tag,
    character::complete::char,
    combinator::{map, opt, value},
    multi::{many0, many1, separated_list0, separated_list1},
    sequence::{delimited, pair, preceded, terminated},
};
use std::fs;
use std::path::Path;

use ast::*;

// ─── Input type ─────────────────────────────────────────────────
type Input<'a> = &'a str;

// ─── Error type ─────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub col: usize,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}:{}: {}", self.line, self.col, self.message)
    }
}

impl std::error::Error for ParseError {}

// ─── Helpers ────────────────────────────────────────────────────

fn ws_char(c: char) -> impl Fn(Input) -> IResult<Input, char> {
    move |input: Input| {
        let (input, _) = skip_ws(input)?;
        char(c)(input)
    }
}

fn skip_ws(input: Input) -> IResult<Input, ()> {
    let mut pos = 0;
    loop {
        while pos < input.len()
            && input[pos..].chars().next().is_some_and(|c| c.is_whitespace())
        {
            pos += 1;
        }
        if input[pos..].starts_with("//") {
            if let Some(end) = input[pos..].find('\n') {
                pos += end + 1;
                continue;
            } else {
                pos = input.len();
                break;
            }
        }
        if input[pos..].starts_with("/*") {
            if let Some(end) = input[pos..].find("*/") {
                pos += end + 2;
                continue;
            } else {
                pos = input.len();
                break;
            }
        }
        break;
    }
    Ok((&input[pos..], ()))
}

fn symbol(s: &'static str) -> impl Fn(Input) -> IResult<Input, Input> {
    move |input: Input| {
        let (input, _) = skip_ws(input)?;
        tag(s)(input)
    }
}

fn keyword(s: &'static str) -> impl Fn(Input) -> IResult<Input, Input> {
    move |input: Input| {
        let (input, _) = skip_ws(input)?;
        let (input, kw) = tag(s)(input)?;
        if let Some(next) = input.chars().next()
            && (next.is_alphanumeric() || next == '_') {
                return Err(nom::Err::Error(nom::error::Error::new(
                    input,
                    nom::error::ErrorKind::Tag,
                )));
            }
        Ok((input, kw))
    }
}

fn keyword_list() -> Vec<&'static str> {
    vec![
        "active", "assert", "atomic", "break", "byte", "chan", "bool", "d_step", "do", "else",
        "enabled", "empty", "fi", "full", "goto", "hidden", "if", "inline", "int", "len", "mtype",
        "nempty", "never", "nfull", "od", "of", "printf", "proctype", "provided", "run", "short",
        "show", "skip", "timeout", "typedef", "unless", "unsigned", "bit",
    ]
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}
#[allow(dead_code)]
fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn ident(input: Input) -> IResult<Input, String> {
    let (input, _) = skip_ws(input)?;
    let (input, raw) = nom::bytes::complete::is_not(" \t\r\n;(){}[]!?:,=+-*/%&|<>^~")(input)?;
    if raw.is_empty() || !is_ident_start(raw.chars().next().unwrap()) {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }
    let kw_list = keyword_list();
    if kw_list.contains(&raw) {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }
    Ok((input, raw.to_string()))
}

// ─── Literals ───────────────────────────────────────────────────
fn int_literal(input: Input) -> IResult<Input, i64> {
    let (input, _) = skip_ws(input)?;
    let (input, sign) = opt(tag("-"))(input)?;
    let (input, digits) = nom::bytes::complete::take_while1(|c: char| c.is_ascii_digit())(input)?;
    let val: i64 = digits.parse().unwrap_or(0);
    Ok((input, if sign.is_some() { -val } else { val }))
}

fn string_literal(input: Input) -> IResult<Input, String> {
    let (input, _) = skip_ws(input)?;
    let (input, s) = delimited(char('"'), nom::bytes::complete::take_while(|c: char| c != '"'), char('"'))(input)?;
    Ok((input, s.to_string()))
}

fn var_type(input: Input) -> IResult<Input, VarType> {
    let (input, _) = skip_ws(input)?;
    alt((
        value(VarType::Bit, keyword("bit")),
        value(VarType::Bool, keyword("bool")),
        value(VarType::Byte, keyword("byte")),
        value(VarType::Short, keyword("short")),
        value(VarType::Int, keyword("int")),
        map(
            preceded(
                keyword("unsigned"),
                opt(delimited(ws_char('('), int_literal, ws_char(')'))),
            ),
            |w| VarType::Unsigned(w.map(|v| v as u32)),
        ),
        value(VarType::Chan, keyword("chan")),
        value(VarType::Mtype, keyword("mtype")),
        map(ident, VarType::Named),
    ))(input)
}

fn array_dim(input: Input) -> IResult<Input, i64> {
    delimited(ws_char('['), int_literal, ws_char(']'))(input)
}

fn var_decl(input: Input) -> IResult<Input, VarDecl> {
    let (input, vt) = var_type(input)?;
    let (input, name) = ident(input)?;
    let (input, arr) = opt(array_dim)(input)?;
    let (input, init) = opt(preceded(symbol("="), expr))(input)?;
    Ok((
        input,
        VarDecl { var_type: vt, name, array_size: arr, init: init.map(Box::new), line: 0 },
    ))
}

// ─── Expressions ────────────────────────────────────────────────
fn expr(input: Input) -> IResult<Input, Expression> {
    disjunction(input)
}

fn disjunction(input: Input) -> IResult<Input, Expression> {
    let (i, first) = conjunction(input)?;
    // Try to parse || if present
    if let Ok((rest, right)) = preceded(symbol("||"), conjunction)(i) {
        Ok((rest, Expression::BinaryOp {
            op: BinaryOp::Or, left: Box::new(first), right: Box::new(right),
        }))
    } else {
        Ok((i, first))
    }
}

fn conjunction(input: Input) -> IResult<Input, Expression> {
    let (input, first) = comparison(input)?;
    if let Ok((rest, right)) = preceded(symbol("&&"), comparison)(input) {
        let result = Expression::BinaryOp {
            op: BinaryOp::And, left: Box::new(first), right: Box::new(right),
        };
        return Ok((rest, result));
    }
    Ok((input, first))
}

fn comparison(input: Input) -> IResult<Input, Expression> {
    let (input, first) = addition(input)?;
    if let Ok((rest, (op, right))) = alt((
        map(pair(symbol("<="), addition), |(_, r)| (BinaryOp::Le, r)),
        map(pair(symbol(">="), addition), |(_, r)| (BinaryOp::Ge, r)),
        map(pair(symbol("!="), addition), |(_, r)| (BinaryOp::Neq, r)),
        map(pair(symbol("=="), addition), |(_, r)| (BinaryOp::Eq, r)),
        map(pair(symbol("<"), addition), |(_, r)| (BinaryOp::Lt, r)),
        map(pair(symbol(">"), addition), |(_, r)| (BinaryOp::Gt, r)),
    ))(input) {
        Ok((
            rest,
            Expression::BinaryOp { op, left: Box::new(first), right: Box::new(right) },
        ))
    } else {
        Ok((input, first))
    }
}

fn addition(input: Input) -> IResult<Input, Expression> {
    let (input, first) = term(input)?;
    if let Ok((rest, (op, right))) = alt((
        map(pair(symbol("+"), term), |(_, r)| (BinaryOp::Add, r)),
        map(pair(symbol("-"), term), |(_, r)| (BinaryOp::Sub, r)),
    ))(input) {
        Ok((
            rest,
            Expression::BinaryOp { op, left: Box::new(first), right: Box::new(right) },
        ))
    } else {
        Ok((input, first))
    }
}

fn term(input: Input) -> IResult<Input, Expression> {
    let (input, first) = unary(input)?;
    if let Ok((rest, (op, right))) = alt((
        map(pair(symbol("*"), unary), |(_, r)| (BinaryOp::Mul, r)),
        map(pair(symbol("/"), unary), |(_, r)| (BinaryOp::Div, r)),
        map(pair(symbol("%"), unary), |(_, r)| (BinaryOp::Mod, r)),
    ))(input) {
        Ok((
            rest,
            Expression::BinaryOp { op, left: Box::new(first), right: Box::new(right) },
        ))
    } else {
        Ok((input, first))
    }
}

fn unary(input: Input) -> IResult<Input, Expression> {
    let (input, _) = skip_ws(input)?;
    alt((
        map(pair(symbol("!"), unary), |(_, e)| Expression::UnaryOp {
            op: UnaryOp::Not, expr: Box::new(e),
        }),
        map(pair(symbol("~"), unary), |(_, e)| Expression::UnaryOp {
            op: UnaryOp::BitNot, expr: Box::new(e),
        }),
        map(pair(symbol("-"), unary), |(_, e)| Expression::UnaryOp {
            op: UnaryOp::Neg, expr: Box::new(e),
        }),
        primary,
    ))(input)
}

fn func_call(input: Input) -> IResult<Input, Expression> {
    let (input, name) = ident(input)?;
    let (input, args) = delimited(
        ws_char('('),
        separated_list0(symbol(","), expr),
        ws_char(')'),
    )(input)?;
    let result = match name.as_str() {
        "len" => Expression::Len(String::new()),
        "full" => Expression::Full(String::new()),
        "empty" => Expression::Empty(String::new()),
        "nfull" => Expression::NFull(String::new()),
        "nempty" => Expression::NEmpty(String::new()),
        "enabled" => Expression::Enabled(String::new()),
        _ => Expression::FuncCall { name, args },
    };
    Ok((input, result))
}

fn primary(input: Input) -> IResult<Input, Expression> {
    let (input, _) = skip_ws(input)?;
    alt((
        map(int_literal, Expression::IntLit),
        map(string_literal, Expression::StringLit),
        map(keyword("true"), |_| Expression::BoolLit(true)),
        map(keyword("false"), |_| Expression::BoolLit(false)),
        map(keyword("timeout"), |_| Expression::Timeout),
        delimited(ws_char('('), expr, ws_char(')')),
        func_call,
        map(ident, Expression::Ident),
    ))(input)
}

// ─── Statements ─────────────────────────────────────────────────
fn stmt(input: Input) -> IResult<Input, Stmt> {
    let (input, _) = skip_ws(input)?;
    alt((
        if_stmt,
        do_stmt,
        goto_stmt,
        break_stmt,
        assert_stmt,
        printf_stmt,
        run_stmt,
        skip_stmt,
        recv_stmt,
        var_decl_stmt,
        assignment_stmt,
        send_stmt,
        expr_stmt,
    ))(input)
}

fn var_decl_stmt(input: Input) -> IResult<Input, Stmt> {
    let (input, decl) = var_decl(input)?;
    let (input, _) = opt(symbol(";"))(input)?;
    Ok((input, Stmt::VarDecl(decl)))
}

fn assignment_stmt(input: Input) -> IResult<Input, Stmt> {
    let (input, target) = ident(input)?;
    let (input, index) = opt(delimited(ws_char('['), expr, ws_char(']')))(input)?;
    let (input, _) = symbol("=")(input)?;
    let (input, value) = expr(input)?;
    let (input, _) = opt(symbol(";"))(input)?;
    Ok((input, Stmt::Assignment {
        target, index: index.map(Box::new), value: Box::new(value), line: 0,
    }))
}
fn expr_stmt(input: Input) -> IResult<Input, Stmt> {
    let (input, e) = expr(input)?;
    let (input, _) = opt(symbol(";"))(input)?;
    Ok((input, Stmt::Expr(e, 0)))
}
fn guard_body(input: Input) -> IResult<Input, Guard> {
    let (input, _) = symbol("::")(input)?;
    if let Ok((rest, _)) = keyword("else")(input) {
        if let Ok((rest2, _)) = opt(symbol("->"))(rest) {
            let (rest3, body) = many0(stmt)(rest2)?;
            return Ok((rest3, Guard { condition: None, body, line: 0 }));
        }
        let (rest3, body) = many0(stmt)(rest)?;
        return Ok((rest3, Guard { condition: None, body, line: 0 }));
    }
    let (input, cond) = opt(expr)(input)?;
    let (input, _) = if cond.is_some() {
        opt(symbol("->"))(input)?
    } else {
        (input, None)
    };
    let (input, body) = many0(stmt)(input)?;
    Ok((input, Guard { condition: cond, body, line: 0 }))
}
fn if_stmt(input: Input) -> IResult<Input, Stmt> {
    let (input, _) = keyword("if")(input)?;
    let (input, guards) = many1(guard_body)(input)?;
    let (input, _) = keyword("fi")(input)?;
    Ok((input, Stmt::If(guards)))
}
fn do_stmt(input: Input) -> IResult<Input, Stmt> {
    let (input, _) = keyword("do")(input)?;
    let (input, guards) = many1(guard_body)(input)?;
    let (input, _) = keyword("od")(input)?;
    Ok((input, Stmt::Do(guards)))
}
fn goto_stmt(input: Input) -> IResult<Input, Stmt> {
    let (input, _) = keyword("goto")(input)?;
    let (input, label) = ident(input)?;
    let (input, _) = opt(symbol(";"))(input)?;
    Ok((input, Stmt::Goto(label, 0)))
}
fn break_stmt(input: Input) -> IResult<Input, Stmt> {
    let (input, _) = keyword("break")(input)?;
    let (input, _) = opt(symbol(";"))(input)?;
    Ok((input, Stmt::Break(0)))
}
fn assert_stmt(input: Input) -> IResult<Input, Stmt> {
    let (input, _) = keyword("assert")(input)?;
    let (input, e) = delimited(ws_char('('), expr, ws_char(')'))(input)?;
    let (input, _) = opt(symbol(";"))(input)?;
    Ok((input, Stmt::Assert(e, 0)))
}
fn printf_stmt(input: Input) -> IResult<Input, Stmt> {
    let (input, _) = keyword("printf")(input)?;
    let (input, (fmt, args)) = delimited(
        ws_char('('),
        pair(string_literal, opt(preceded(symbol(","), separated_list0(symbol(","), expr)))),
        ws_char(')'),
    )(input)?;
    let (input, _) = opt(symbol(";"))(input)?;
    Ok((input, Stmt::Printf(fmt, args.unwrap_or_default(), 0)))
}
fn skip_stmt(input: Input) -> IResult<Input, Stmt> {
    let (input, _) = keyword("skip")(input)?;
    let (input, _) = opt(symbol(";"))(input)?;
    Ok((input, Stmt::Skip(0)))
}
fn send_stmt(input: Input) -> IResult<Input, Stmt> {
    let (input, channel) = ident(input)?;
    let (input, _) = symbol("!")(input)?;
    let (input, target_val) = alt((
        map(expr, SendTarget::Value),
        map(ident, SendTarget::Ident),
    ))(input)?;
    let (input, args) = opt(delimited(ws_char('('), separated_list0(symbol(","), expr), ws_char(')')))(input)?;
    let (input, _) = opt(symbol(";"))(input)?;
    Ok((input, Stmt::Send {
        channel, target: target_val, args: args.unwrap_or_default(), line: 0,
    }))
}
fn recv_stmt(input: Input) -> IResult<Input, Stmt> {
    let (input, channel) = ident(input)?;
    let (input, _) = symbol("?")(input)?;
    let (input, target) = alt((
        map(delimited(ws_char('['), expr, ws_char(']')), RecvTarget::Eval),
        map(separated_list1(symbol(","), ident), RecvTarget::VarList),
    ))(input)?;
    let (input, _) = opt(symbol(";"))(input)?;
    Ok((input, Stmt::Recv { channel, target, line: 0 }))
}
fn run_stmt(input: Input) -> IResult<Input, Stmt> {
    let (input, _) = keyword("run")(input)?;
    let (input, name) = ident(input)?;
    let (input, args) = opt(delimited(
        ws_char('('),
        separated_list0(symbol(","), expr),
        ws_char(')'),
    ))(input)?;
    let (input, _) = opt(symbol(";"))(input)?;
    Ok((input, Stmt::Run(name, args.unwrap_or_default(), 0)))
}
fn proctype_def(input: Input) -> IResult<Input, TopLevel> {
    let (input, _) = skip_ws(input)?;
    let (input, active) = opt(keyword("active"))(input)?;
    let (input, _) = keyword("proctype")(input)?;
    let (input, name) = ident(input)?;
    let (input, params) = delimited(
        ws_char('('),
        opt(separated_list0(symbol(","), var_decl)),
        ws_char(')'),
    )(input)?;
    let (input, body) = delimited(ws_char('{'), many0(stmt), ws_char('}'))(input)?;
    Ok((input, TopLevel::Proctype(ProctypeDef {
        name, active: active.is_some(), provided: None,
        parameters: params.unwrap_or_default(), body, line: 0,
    })))
}
fn init_def(input: Input) -> IResult<Input, TopLevel> {
    let (input, _) = keyword("init")(input)?;
    let (input, body) = delimited(ws_char('{'), many0(stmt), ws_char('}'))(input)?;
    Ok((input, TopLevel::Init(InitDef { body, line: 0 })))
}
fn never_claim(input: Input) -> IResult<Input, TopLevel> {
    let (input, _) = keyword("never")(input)?;
    let (input, body) = delimited(ws_char('{'), many0(stmt), ws_char('}'))(input)?;
    Ok((input, TopLevel::NeverClaim(NeverClaim { body, line: 0 })))
}
fn ltl_formula(input: Input) -> IResult<Input, TopLevel> {
    let (input, _) = keyword("ltl")(input)?;
    let (input, name) = opt(ident)(input)?;
    let (input, _) = ws_char('{')(input)?;
    let (input, formula) = nom::bytes::complete::take_while(|c: char| c != '}')(input)?;
    let (input, _) = ws_char('}')(input)?;
    Ok((input, TopLevel::Ltl(LtlFormula {
        name, formula: formula.trim().to_string(), line: 0,
    })))
}
fn preprocessor(input: Input) -> IResult<Input, TopLevel> {
    let (input, _) = skip_ws(input)?;
    if !input.starts_with('#') {
        return Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)));
    }
    let (input, content) = nom::bytes::complete::take_while(|c: char| c != '\n')(&input[1..])?;
    let (input, _) = opt(char('\n'))(input)?;
    Ok((input, TopLevel::PreprocessorDirective(format!("#{}", content))))
}
fn top_level(input: Input) -> IResult<Input, TopLevel> {
    let (input, _) = skip_ws(input)?;
    alt((
        proctype_def,
        init_def,
        never_claim,
        ltl_formula,
        preprocessor,
        map(terminated(var_decl, symbol(";")), TopLevel::GlobalVar),
    ))(input)
}
pub fn parse(source: &str) -> anyhow::Result<PromelaModel> {
    let (_, declarations) = many0(top_level)(source)
        .map_err(|e| anyhow::anyhow!("parse error: {:?}", e))?;
    Ok(PromelaModel {
        declarations,
        source: Some(source.to_string()),
    })
}
pub fn parse_file(path: &Path) -> anyhow::Result<PromelaModel> {
    let source = fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("cannot read file {}: {}", path.display(), e))?;
    parse(&source)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_basic_var_decl() {
        let source = "byte x; bit flag; int counter = 0;";
        let model = parse(source).unwrap();
        assert_eq!(model.declarations.len(), 3);
    }
    #[test]
    fn test_active_proctype() {
        let source = "active proctype P() { byte x; x = 1; }";
        let model = parse(source).unwrap();
        assert_eq!(model.declarations.len(), 1);
        match &model.declarations[0] {
            TopLevel::Proctype(p) => {
                assert!(p.active);
                assert_eq!(p.name, "P");
            }
            _ => panic!("expected proctype"),
        }
    }
    #[test]
    fn test_if_fi() {
        let source = "active proctype P() {\n    if\n    :: (x > 0) -> y = 1\n    :: else -> y = 0\n    fi\n}";
        let model = parse(source).unwrap();
        match &model.declarations[0] {
            TopLevel::Proctype(p) => match &p.body[0] {
                Stmt::If(guards) => assert_eq!(guards.len(), 2),
                _ => panic!("expected if"),
            },
            _ => panic!("expected proctype"),
        }
    }
    #[test]
    fn test_do_od() {
        let source = "active proctype P() {\n    do\n    :: (x > 0) -> x = x - 1\n    :: (x == 0) -> break\n    od\n}";
        let model = parse(source).unwrap();
        match &model.declarations[0] {
            TopLevel::Proctype(p) => match &p.body[0] {
                Stmt::Do(guards) => assert_eq!(guards.len(), 2),
                _ => panic!("expected do"),
            },
            _ => panic!("expected proctype"),
        }
    }
    #[test]
    fn test_ltl_formula() {
        let source = "ltl p0 { [](x == 0) }";
        let model = parse(source).unwrap();
        assert_eq!(model.declarations.len(), 1);
        match &model.declarations[0] {
            TopLevel::Ltl(l) => {
                assert_eq!(l.name.as_deref(), Some("p0"));
                assert!(l.formula.contains("[]"));
            }
            _ => panic!("expected LTL"),
        }
    }
    #[test]
    fn test_preprocessor() {
        let source = "#define N 5\nbyte x;\n";
        let model = parse(source).unwrap();
        assert_eq!(model.declarations.len(), 2);
        match &model.declarations[0] {
            TopLevel::PreprocessorDirective(d) => assert!(d.contains("define")),
            _ => panic!("expected preprocessor"),
        }
    }
    #[test]
    fn test_channel_send_recv() {
        let source = "active proctype P() {\n    ch!msg(1);\n    ch?msg;\n}";
        let model = parse(source).unwrap();
        assert_eq!(model.declarations.len(), 1);
    }
}
