//! Promela parser: converts Promela source text into an AST using nom.

pub mod ast;

use nom::{
    IResult,
    branch::alt,
    combinator::{map, opt},
    multi::{many0, many1, separated_list0, separated_list1},
    sequence::{delimited, pair, preceded},
};
use std::fs;
use std::path::Path;

use ast::*;

pub(crate) use self::expressions::*;
pub(crate) use self::helpers::*;
pub(crate) use self::literals::*;
pub(crate) use self::top_defs::*;

// ─── Input type ─────────────────────────────────────────────────
/// Parser input type: a borrowed string slice.
pub(crate) type Input<'a> = &'a str;

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
mod expressions;
mod helpers;
mod literals;
mod top_defs;

// ─── Declarations ──────────────────────────────────────────────

pub(crate) fn array_dim(input: Input) -> IResult<Input, i64> {
    delimited(ws_char('['), int_literal, ws_char(']'))(input)
}

pub(crate) fn var_decl(input: Input) -> IResult<Input, VarDecl> {
    let (input, vt) = var_type(input)?;
    let (input, name) = ident(input)?;
    let (input, arr) = opt(array_dim)(input)?;
    let (input, init) = opt(preceded(symbol("="), expr))(input)?;
    Ok((
        input,
        VarDecl {
            var_type: vt,
            name,
            array_size: arr,
            init: init.map(Box::new),
            line: 0,
        },
    ))
}

// ─── Statements ─────────────────────────────────────────────────
fn stmt(input: Input) -> IResult<Input, Stmt> {
    let (input, _) = skip_ws(input)?;
    alt((
        if_stmt,
        do_stmt,
        dstep_stmt,
        atomic_stmt,
        goto_stmt,
        break_stmt,
        assert_stmt,
        printf_stmt,
        run_stmt,
        for_stmt,
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
    Ok((
        input,
        Stmt::Assignment {
            target,
            index: index.map(Box::new),
            value: Box::new(value),
            line: 0,
        },
    ))
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
            return Ok((
                rest3,
                Guard {
                    condition: None,
                    body,
                    line: 0,
                },
            ));
        }
        let (rest3, body) = many0(stmt)(rest)?;
        return Ok((
            rest3,
            Guard {
                condition: None,
                body,
                line: 0,
            },
        ));
    }

    // Check if this looks like a statement (assignment, send, recv) before trying condition
    // Pattern: ident followed by =, !, ?, or [ followed by =, !, ?
    let is_stmt_start = {
        let trimmed = input.trim_start();
        // Check if it starts with identifier followed by statement operator
        let mut chars = trimmed.chars().peekable();

        // Collect identifier characters
        while let Some(&ch) = chars.peek() {
            if ch.is_alphanumeric() || ch == '_' {
                chars.next();
            } else {
                break;
            }
        }

        if chars.peek().copied() == Some('[') {
            // Array access: ident[expr] =, ident[expr] !, or ident[expr] ?
            // Skip the bracket expression to check what follows
            chars.next(); // skip '['
            let mut depth = 1;
            while depth > 0 {
                match chars.next() {
                    Some('[') => depth += 1,
                    Some(']') => depth -= 1,
                    Some(_) => {},
                    None => break,
                }
            }
        }

        // Skip whitespace after identifier (or after brackets)
        while let Some(&ch) = chars.peek() {
            if ch.is_whitespace() {
                chars.next();
            } else {
                break;
            }
        }

        // Check what comes after
        matches!(chars.peek(), Some('=') | Some('!') | Some('?'))
    };

    // If it looks like a statement, parse it as such (no condition)
    // Parse body statements but stop before :: (next guard) or od/fi
    if is_stmt_start {
        let mut body = Vec::new();
        let mut remaining = input;
        loop {
            // Check if we're at a guard separator (::) or end marker (od/fi)
            let peek = remaining.trim_start();
            if peek.starts_with("::") || peek.starts_with("od") || peek.starts_with("fi") {
                break;
            }
            match stmt(remaining) {
                Ok((rest, s)) => {
                    body.push(s);
                    remaining = rest;
                }
                Err(_) => break,
            }
        }
        return Ok((
            remaining,
            Guard {
                condition: None,
                body,
                line: 0,
            },
        ));
    }

    // Try to parse: [condition] -> [body]
    let (input, cond) = opt(expr)(input)?;
    let (input, has_arrow) = if cond.is_some() {
        let (rest, arrow) = opt(symbol("->"))(input)?;
        (rest, arrow.is_some())
    } else {
        (input, false)
    };

    let (condition, body_input) = if cond.is_some() && !has_arrow {
        // Condition without arrow - treat as condition
        (cond, input)
    } else {
        (cond, input)
    };

    // Parse body statements
    let (input, body) = many0(stmt)(body_input)?;

    Ok((
        input,
        Guard {
            condition,
            body,
            line: 0,
        },
    ))
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
        pair(
            string_literal,
            opt(preceded(symbol(","), separated_list0(symbol(","), expr))),
        ),
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
fn dstep_stmt(input: Input) -> IResult<Input, Stmt> {
    let (input, _) = keyword("d_step")(input)?;
    let (input, body) = delimited(ws_char('{'), many0(stmt), ws_char('}'))(input)?;
    let (input, _) = opt(symbol(";"))(input)?;
    Ok((input, Stmt::DStep(body, 0)))
}
fn atomic_stmt(input: Input) -> IResult<Input, Stmt> {
    let (input, _) = keyword("atomic")(input)?;
    let (input, body) = delimited(ws_char('{'), many0(stmt), ws_char('}'))(input)?;
    let (input, _) = opt(symbol(";"))(input)?;
    Ok((input, Stmt::Atomic(body, 0)))
}
/// Parse a channel reference: either a simple identifier or an indexed expression.
/// Supports `tok` (simple) and `tok[i]` (indexed) syntax.
fn channel_expr(input: Input) -> IResult<Input, Expression> {
    let (input, name) = ident(input)?;
    let (input, index) = opt(delimited(ws_char('['), expr, ws_char(']')))(input)?;
    match index {
        Some(idx) => Ok((
            input,
            Expression::ArrayAccess {
                name: name.to_string(),
                index: Box::new(idx),
            },
        )),
        None => Ok((input, Expression::Ident(name.to_string()))),
    }
}

fn send_stmt(input: Input) -> IResult<Input, Stmt> {
    let (input, channel) = channel_expr(input)?;
    let (input, _) = symbol("!")(input)?;
    let (input, target_val) =
        alt((map(expr, SendTarget::Value), map(ident, SendTarget::Ident)))(input)?;
    let (input, args) = opt(delimited(
        ws_char('('),
        separated_list0(symbol(","), expr),
        ws_char(')'),
    ))(input)?;
    let (input, _) = opt(symbol(";"))(input)?;
    Ok((
        input,
        Stmt::Send {
            channel: Box::new(channel),
            target: target_val,
            args: args.unwrap_or_default(),
            line: 0,
        },
    ))
}
fn recv_stmt(input: Input) -> IResult<Input, Stmt> {
    let (input, channel) = channel_expr(input)?;
    let (input, _) = symbol("?")(input)?;
    let (input, target) = alt((
        map(
            delimited(ws_char('['), expr, ws_char(']')),
            RecvTarget::Eval,
        ),
        map(separated_list1(symbol(","), ident), RecvTarget::VarList),
        // Support `? expr` for receive-and-match (e.g., `ch ? 0`)
        map(expr, RecvTarget::Eval),
    ))(input)?;
    let (input, _) = opt(symbol(";"))(input)?;
    Ok((
        input,
        Stmt::Recv {
            channel: Box::new(channel),
            target,
            line: 0,
        },
    ))
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
fn for_stmt(input: Input) -> IResult<Input, Stmt> {
    let (input, _) = keyword("for")(input)?;
    let (input, _) = ws_char('(')(input)?;
    let (input, init) = var_decl_stmt(input)?;
    let (input, _) = symbol(";")(input)?;
    let (input, condition) = expr(input)?;
    let (input, _) = symbol(";")(input)?;
    let (input, update) = assignment_stmt(input)?;
    let (input, _) = ws_char(')')(input)?;
    let (input, body) = delimited(ws_char('{'), many0(stmt), ws_char('}'))(input)?;
    Ok((
        input,
        Stmt::For {
            init: Box::new(init),
            condition,
            update: Box::new(update),
            body,
            line: 0,
        },
    ))
}
pub fn parse(source: &str) -> anyhow::Result<PromelaModel> {
    let (_, declarations) =
        many0(top_level)(source).map_err(|e| anyhow::anyhow!("parse error: {:?}", e))?;
    let declarations: Vec<TopLevel> = declarations.into_iter().flatten().collect();
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
mod tests;
