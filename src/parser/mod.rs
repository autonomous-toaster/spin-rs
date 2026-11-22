//! Promela parser: converts Promela source text into an AST using nom.

pub mod ast;

use nom::{
    IResult,
    branch::alt,
    combinator::{map, opt},
    multi::{many0, many1, separated_list0, separated_list1},
    sequence::{delimited, pair, preceded, terminated},
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
mod preprocess;
mod top_defs;

// ─── Declarations ──────────────────────────────────────────────

pub(crate) fn array_dim(input: Input) -> IResult<Input, i64> {
    delimited(ws_char('['), int_literal, ws_char(']'))(input)
}

pub(crate) fn var_decl(input: Input) -> IResult<Input, VarDecl> {
    let (input, vt) = var_type(input)?;
    // A named mtype instance: `mtype:fruit b` declares `b` with the type `mtype:fruit`. The
    // qualifier names which mtype, and the variable's own type is still mtype.
    let (input, _) = opt(|i| {
        let (i, _) = ws_char(':')(i)?;
        ident(i)
    })(input)?;
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
/// Parse one statement, and the `->` that may follow it.
///
/// Promela separates statements with `->` as often as with `;`: `b = 1 -> a = 1` is ordinary
/// inside a block, an atomic region and a body. Only some statement parsers absorbed a semicolon
/// and none absorbed an arrow, so a body written that way failed to parse.
fn stmt(input: Input) -> IResult<Input, Stmt> {
    let (input, stmt) = stmt_inner(input)?;
    let (input, _) = opt(alt((symbol("->"), symbol(";"))))(input)?;
    Ok((input, stmt))
}

fn stmt_inner(input: Input) -> IResult<Input, Stmt> {
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
        incr_stmt,
        assignment_stmt,
        send_stmt,
        label_stmt,
        expr_stmt,
    ))(input)
}

fn var_decl_stmt(input: Input) -> IResult<Input, Stmt> {
    let (input, decl) = var_decl(input)?;
    let (input, _) = opt(symbol(";"))(input)?;
    Ok((input, Stmt::VarDecl(decl)))
}

/// `x++`, `x--`, `++x`, `--x`: an increment written as a statement.
///
/// Parsed before `assignment_stmt`, which would otherwise read `x` as a bare expression and leave
/// the operator unconsumed, taking the enclosing proctype down with it.
fn incr_stmt(input: Input) -> IResult<Input, Stmt> {
    fn build(name: String, delta: i64) -> Stmt {
        Stmt::Assignment {
            target: name.clone(),
            index: None,
            field: None,
            value: Box::new(Expression::BinaryOp {
                op: if delta > 0 {
                    BinaryOp::Add
                } else {
                    BinaryOp::Sub
                },
                left: Box::new(Expression::Ident(name)),
                right: Box::new(Expression::IntLit(delta.abs())),
            }),
            line: 0,
        }
    }
    alt((
        map(preceded(symbol("++"), ident), |name| build(name, 1)),
        map(preceded(symbol("--"), ident), |name| build(name, -1)),
        map(terminated(ident, symbol("++")), |name| build(name, 1)),
        map(terminated(ident, symbol("--")), |name| build(name, -1)),
    ))(input)
}

fn assignment_stmt(input: Input) -> IResult<Input, Stmt> {
    // Try to parse as record access first: ident.field = expr
    if let Ok((rest, (target, field))) = pair(ident, preceded(ws_char('.'), ident))(input)
        && let Ok((rest2, _)) = symbol("=")(rest)
    {
        // This is a record assignment
        let (rest3, val) = expr(rest2)?;
        let (rest4, _) = opt(symbol(";"))(rest3)?;
        return Ok((
            rest4,
            Stmt::Assignment {
                target,
                index: None,
                field: Some(field),
                value: Box::new(val),
                line: 0,
            },
        ));
    }

    // Regular assignment: ident[expr] = expr or ident = expr
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
            field: None,
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

    // A guard body led by a statement keyword is a statement, never a condition.
    // Without this, a guard body of just "skip" parses as the condition `skip`,
    // which reaches Lua as an undefined global and raises at runtime.
    let starts_with_stmt_keyword = {
        let trimmed = input.trim_start();
        const STMT_KEYWORDS: &[&str] = &[
            "skip", "break", "goto", "assert", "printf", "run", "atomic", "d_step", "if", "do",
            "for", "unless", "bool", "bit", "byte", "short", "int", "mtype", "chan", "unsigned",
            "hidden", "typedef",
        ];
        STMT_KEYWORDS.iter().any(|kw| {
            trimmed.starts_with(kw)
                && !trimmed[kw.len()..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_')
        })
    };

    // Check if this looks like a statement (assignment, send, recv) before trying condition
    // Pattern: ident followed by =, !, ?, or [=!?, or ident(
    let is_stmt_start = !starts_with_stmt_keyword && {
        let trimmed = input.trim_start();
        let mut chars = trimmed.chars().peekable();
        let mut has_ident = false;

        // Collect identifier characters
        while let Some(&ch) = chars.peek() {
            if ch.is_alphanumeric() || ch == '_' {
                has_ident = true;
                chars.next();
            } else {
                break;
            }
        }

        if has_ident && chars.peek().copied() == Some('(') {
            // Function call: ident(args)
            true
        } else if has_ident && chars.peek().copied() == Some('[') {
            // Array access: ident[expr] =, ident[expr] !, or ident[expr] ?
            chars.next(); // skip '['
            let mut depth = 1;
            while depth > 0 {
                match chars.next() {
                    Some('[') => depth += 1,
                    Some(']') => depth -= 1,
                    Some(_) => {}
                    None => break,
                }
            }
            while let Some(&ch) = chars.peek() {
                if ch.is_whitespace() {
                    chars.next();
                } else {
                    break;
                }
            }
            matches!(chars.peek(), Some('=') | Some('!') | Some('?'))
        } else if has_ident {
            while let Some(&ch) = chars.peek() {
                if ch.is_whitespace() {
                    chars.next();
                } else {
                    break;
                }
            }
            // A single '=' starts an assignment; '==' is a comparison, so the text
            // is a guard condition. Likewise '!=' compares while '!' sends. Treating
            // '==' as '=' swallows the arrow of ":: done_t0_9 == 1 -> ...", making
            // every veriplan-generated proctype unparsable.
            let mut probe = chars.clone();
            match probe.next() {
                Some('=') => probe.next() != Some('='),
                Some('!') => probe.next() != Some('='),
                Some('?') => true,
                _ => false,
            }
        } else {
            false
        }
    };

    // If it looks like a statement, parse it as such (no condition)
    // Parse body statements but stop before :: (next guard) or od/fi
    if is_stmt_start {
        let mut body = Vec::new();
        let mut remaining = input;
        // The first statement is the guard and the arrow separates it from the body:
        // `:: c ? x -> a = 1` is an option that is enabled when `c` has a message. Without
        // consuming the arrow here the guard body ended at the receive and the enclosing
        // `if` then choked on `->`, which is why a receive could never be written as a guard.
        if let Ok((rest, s)) = stmt(remaining) {
            body.push(s);
            if let Ok((rest, _)) = opt(symbol("->"))(rest) {
                remaining = rest;
            }
        }
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
    // Promela permits a trailing semicolon after fi: `if :: a = 1 fi;`
    let (input, _) = opt(symbol(";"))(input)?;
    Ok((input, Stmt::If(guards)))
}
fn do_stmt(input: Input) -> IResult<Input, Stmt> {
    let (input, _) = keyword("do")(input)?;
    let (input, guards) = many1(guard_body)(input)?;
    let (input, _) = keyword("od")(input)?;
    // Promela permits a trailing semicolon after od: `do :: a = 1 od;`
    let (input, _) = opt(symbol(";"))(input)?;
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
fn label_stmt(input: Input) -> IResult<Input, Stmt> {
    let (input, name) = ident(input)?;
    let (input, _) = ws_char(':')(input)?;
    // A label is followed by a statement (or another label)
    // We parse the label and let the caller handle the following statement
    let (input, _) = opt(symbol(";"))(input)?;
    Ok((input, Stmt::Label(name, 0)))
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
    // Try `!!` (sorted send) first, then `!` (regular send)
    let (input, is_sorted) =
        alt((map(symbol("!!"), |_| true), map(symbol("!"), |_| false)))(input)?;
    let (input, target_val) = if is_sorted {
        map(expr, SendTarget::Sorted)(input)?
    } else {
        alt((map(expr, SendTarget::Value), map(ident, SendTarget::Ident)))(input)?
    };
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
    // Try `??` (random receive) first, then `?` (regular receive)
    let (input, is_random) =
        alt((map(symbol("??"), |_| true), map(symbol("?"), |_| false)))(input)?;
    let (input, target) = if is_random {
        map(separated_list1(symbol(","), ident), RecvTarget::Random)(input)?
    } else {
        alt((
            // `?[expr]` - poll receive: check without consuming
            map(
                delimited(ws_char('['), expr, ws_char(']')),
                RecvTarget::Poll,
            ),
            // `? eval(expr)` - eval receive: consume only if matches
            map(
                preceded(keyword("eval"), delimited(ws_char('('), expr, ws_char(')'))),
                RecvTarget::Eval,
            ),
            map(separated_list1(symbol(","), ident), RecvTarget::VarList),
            // `? expr` for receive-and-match (e.g., `ch ? 0`)
            map(expr, RecvTarget::Eval),
        ))(input)?
    };
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

    // Try Promela-style first, then C-style
    let (input, stmt) = alt((for_promela_style, for_c_style))(input)?;
    // Every other statement parser absorbs an optional trailing semicolon; without this one a
    // `for` followed by another statement made the whole proctype fail to parse.
    let (input, _) = opt(symbol(";"))(input)?;
    Ok((input, stmt))
}

/// Parse Promela-style `for (var : start .. end) { body }` and expand at parse time.
///
/// The separator is a colon in Promela; this accepted only `in`, so every valid Promela for-loop
/// was rejected as a parse error. Both are accepted now.
fn for_promela_style(input: Input) -> IResult<Input, Stmt> {
    let (input, var_name) = ident(input)?;
    let (input, _) = alt((symbol("in"), symbol(":")))(input)?;
    let (input, start_val) = int_literal(input)?;
    let (input, _) = symbol("..")(input)?;
    let (input, end_val) = int_literal(input)?;
    let (input, _) = ws_char(')')(input)?;
    let (input, body) = delimited(ws_char('{'), many0(stmt), ws_char('}'))(input)?;
    // `for (i : a .. b) body` is a loop, not an atomic region: pan executes the body's statements
    // as ordinary steps, so a state between two of them is reachable and observable. Expanding it
    // to `atomic` made `for (i : 0 .. 3) { x = i; y = i }` unable to reach x != y, which pan
    // reaches and refutes. It becomes the equivalent init/condition/update, which the code
    // generator already knows how to flatten.
    let var = || Box::new(Expression::Ident(var_name.clone()));
    let assign = |value: Expression| {
        Box::new(Stmt::Assignment {
            target: var_name.clone(),
            index: None,
            field: None,
            value: Box::new(value),
            line: 0,
        })
    };
    Ok((
        input,
        Stmt::For {
            init: assign(Expression::IntLit(start_val)),
            condition: Expression::BinaryOp {
                op: BinaryOp::Le,
                left: var(),
                right: Box::new(Expression::IntLit(end_val)),
            },
            update: assign(Expression::BinaryOp {
                op: BinaryOp::Add,
                left: var(),
                right: Box::new(Expression::IntLit(1)),
            }),
            body,
            line: 0,
        },
    ))
}

/// Parse C-style: for (init; condition; update) { body }
fn for_c_style(input: Input) -> IResult<Input, Stmt> {
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
/// Parse a single expression.
///
/// Used to decide an atomic proposition that names no single state variable, such as
/// `a == b`: the atom is an expression, and the alternative is to leave it undecided.
pub fn parse_expression(source: &str) -> anyhow::Result<Expression> {
    let trimmed = source.trim();
    match expressions::expr(trimmed) {
        Ok((rest, e)) if rest.trim().is_empty() => Ok(e),
        Ok((rest, _)) => anyhow::bail!(
            "expression \"{}\" has {} character(s) left over",
            trimmed,
            rest.trim().len()
        ),
        Err(e) => anyhow::bail!("cannot parse expression \"{}\": {:?}", trimmed, e),
    }
}

pub fn parse(source: &str) -> anyhow::Result<PromelaModel> {
    // Spin preprocesses before it parses, so a macro is part of the language rather than a
    // pre-pass a caller is expected to do. A directive this implementation does not support is an
    // error naming it, never a silently different model.
    let expanded =
        preprocess::preprocess(source).map_err(|e| anyhow::anyhow!("preprocessor: {}", e))?;
    let source: &str = &expanded;
    let (rest, declarations) =
        many0(top_level)(source).map_err(|e| anyhow::anyhow!("parse error: {:?}", e))?;
    let declarations: Vec<TopLevel> = declarations.into_iter().flatten().collect();

    // A partially parsed model must never be verified: leftover input means the
    // grammar did not understand part of the source, so any verdict derived from
    // the truncated model would be unsound.
    // Strip trailing whitespace and comments: a file may legitimately end with them.
    let leftover = match skip_ws(rest) {
        Ok((r, _)) => r.trim(),
        Err(_) => rest.trim(),
    };
    if !leftover.is_empty() {
        let snippet: String = leftover.chars().take(120).collect();
        return Err(anyhow::anyhow!(
            "parse error: {} character(s) left unconsumed at top level near '{}'",
            leftover.len(),
            snippet
        ));
    }

    // Collect mtype names and assign sequential IDs
    let mut mtype_names = std::collections::HashMap::new();
    let mut next_id = 0i64;
    for decl in &declarations {
        if let TopLevel::MtypeDecl { names, .. } = decl {
            for name in names {
                if !mtype_names.contains_key(name) {
                    mtype_names.insert(name.clone(), next_id);
                    next_id += 1;
                }
            }
        }
    }

    // Collect typedef definitions
    let mut typedefs = std::collections::HashMap::new();
    for decl in &declarations {
        if let TopLevel::Typedef { name, fields, .. } = decl {
            typedefs.insert(name.clone(), fields.clone());
        }
    }

    let mut model = PromelaModel {
        declarations,
        source: Some(source.to_string()),
        mtype_names,
        typedefs,
    };

    // Post-process: replace mtype name identifiers with integer literals
    model.resolve_mtype_refs();

    Ok(model)
}
pub fn parse_file(path: &Path) -> anyhow::Result<PromelaModel> {
    let source = fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("cannot read file {}: {}", path.display(), e))?;
    parse(&source)
}

#[cfg(test)]
mod tests;
