//! Expression parsers: arithmetic, comparison, logical, function calls.

use nom::{
    branch::alt,
    combinator::{map, opt},
    multi::separated_list0,
    sequence::{delimited, pair, preceded},
    IResult,
};

use super::ast::*;
use super::helpers::*;
use super::literals::*;
use super::Input;

// ─── Expressions ────────────────────────────────────────────────
pub(crate) fn expr(input: Input) -> IResult<Input, Expression> {
    disjunction(input)
}

pub(crate) fn disjunction(input: Input) -> IResult<Input, Expression> {
    let (mut input, mut left) = conjunction(input)?;
    // Handle multiple || operators (left-associative)
    while let Ok((rest, right)) = preceded(symbol("||"), conjunction)(input) {
        left = Expression::BinaryOp {
            op: BinaryOp::Or,
            left: Box::new(left),
            right: Box::new(right),
        };
        input = rest;
    }
    Ok((input, left))
}

pub(crate) fn conjunction(input: Input) -> IResult<Input, Expression> {
    let (mut input, mut left) = comparison(input)?;
    // Handle multiple && operators (left-associative)
    while let Ok((rest, right)) = preceded(symbol("&&"), comparison)(input) {
        left = Expression::BinaryOp {
            op: BinaryOp::And,
            left: Box::new(left),
            right: Box::new(right),
        };
        input = rest;
    }
    Ok((input, left))
}

pub(crate) fn comparison(input: Input) -> IResult<Input, Expression> {
    let (input, first) = addition(input)?;
    if let Ok((rest, (op, right))) = alt((
        map(pair(symbol("<="), addition), |(_, r)| (BinaryOp::Le, r)),
        map(pair(symbol(">="), addition), |(_, r)| (BinaryOp::Ge, r)),
        map(pair(symbol("!="), addition), |(_, r)| (BinaryOp::Neq, r)),
        map(pair(symbol("=="), addition), |(_, r)| (BinaryOp::Eq, r)),
        map(pair(symbol("<"), addition), |(_, r)| (BinaryOp::Lt, r)),
        map(pair(symbol(">"), addition), |(_, r)| (BinaryOp::Gt, r)),
    ))(input)
    {
        Ok((
            rest,
            Expression::BinaryOp {
                op,
                left: Box::new(first),
                right: Box::new(right),
            },
        ))
    } else {
        Ok((input, first))
    }
}

pub(crate) fn addition(input: Input) -> IResult<Input, Expression> {
    let (input, first) = term(input)?;
    if let Ok((rest, (op, right))) = alt((
        map(pair(symbol("+"), term), |(_, r)| (BinaryOp::Add, r)),
        map(pair(symbol("-"), term), |(_, r)| (BinaryOp::Sub, r)),
    ))(input)
    {
        Ok((
            rest,
            Expression::BinaryOp {
                op,
                left: Box::new(first),
                right: Box::new(right),
            },
        ))
    } else {
        Ok((input, first))
    }
}

pub(crate) fn term(input: Input) -> IResult<Input, Expression> {
    let (input, first) = unary(input)?;
    if let Ok((rest, (op, right))) = alt((
        map(pair(symbol("*"), unary), |(_, r)| (BinaryOp::Mul, r)),
        map(pair(symbol("/"), unary), |(_, r)| (BinaryOp::Div, r)),
        map(pair(symbol("%"), unary), |(_, r)| (BinaryOp::Mod, r)),
    ))(input)
    {
        Ok((
            rest,
            Expression::BinaryOp {
                op,
                left: Box::new(first),
                right: Box::new(right),
            },
        ))
    } else {
        Ok((input, first))
    }
}

pub(crate) fn unary(input: Input) -> IResult<Input, Expression> {
    let (input, _) = skip_ws(input)?;
    alt((
        map(pair(symbol("!"), unary), |(_, e)| Expression::UnaryOp {
            op: UnaryOp::Not,
            expr: Box::new(e),
        }),
        map(pair(symbol("~"), unary), |(_, e)| Expression::UnaryOp {
            op: UnaryOp::BitNot,
            expr: Box::new(e),
        }),
        map(pair(symbol("-"), unary), |(_, e)| Expression::UnaryOp {
            op: UnaryOp::Neg,
            expr: Box::new(e),
        }),
        primary,
    ))(input)
}

pub(crate) fn func_call(input: Input) -> IResult<Input, Expression> {
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

pub(crate) fn primary(input: Input) -> IResult<Input, Expression> {
    let (input, _) = skip_ws(input)?;
    alt((
        map(int_literal, Expression::IntLit),
        map(string_literal, Expression::StringLit),
        map(keyword("true"), |_| Expression::BoolLit(true)),
        map(keyword("false"), |_| Expression::BoolLit(false)),
        map(keyword("timeout"), |_| Expression::Timeout),
        delimited(ws_char('('), expr, ws_char(')')),
        // Remote reference: P@x
        map(pair(ident, preceded(symbol("@"), ident)), |(pid, name)| {
            Expression::RemoteRef {
                pid: Box::new(Expression::Ident(pid)),
                name,
            }
        }),
        // Function call: ident(args) — must come before array-access to consume `(` first
        func_call,
        // Array access or plain ident: ident[expr] or ident
        map(
            pair(ident, opt(delimited(ws_char('['), expr, ws_char(']')))),
            |(name, index)| match index {
                Some(idx) => Expression::ArrayAccess {
                    name,
                    index: Box::new(idx),
                },
                None => Expression::Ident(name),
            },
        ),
    ))(input)
}
