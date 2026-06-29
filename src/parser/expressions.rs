//! Expression parsers: arithmetic, comparison, logical, function calls.

use nom::{
    IResult,
    branch::alt,
    combinator::{map, opt},
    multi::separated_list0,
    sequence::{delimited, pair, preceded},
};

use super::Input;
use super::ast::*;
use super::helpers::*;
use super::literals::*;

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
        "len" => {
            if let Some(arg) = args.first() {
                match arg {
                    Expression::Ident(s) => Expression::Len(s.clone()),
                    _ => Expression::FuncCall { name, args },
                }
            } else {
                Expression::FuncCall { name, args }
            }
        }
        "full" => {
            if let Some(arg) = args.first() {
                match arg {
                    Expression::Ident(s) => Expression::Full(s.clone()),
                    _ => Expression::FuncCall { name, args },
                }
            } else {
                Expression::FuncCall { name, args }
            }
        }
        "empty" => {
            if let Some(arg) = args.first() {
                match arg {
                    Expression::Ident(s) => Expression::Empty(s.clone()),
                    _ => Expression::FuncCall { name, args },
                }
            } else {
                Expression::FuncCall { name, args }
            }
        }
        "nfull" => {
            if let Some(arg) = args.first() {
                match arg {
                    Expression::Ident(s) => Expression::NFull(s.clone()),
                    _ => Expression::FuncCall { name, args },
                }
            } else {
                Expression::FuncCall { name, args }
            }
        }
        "nempty" => {
            if let Some(arg) = args.first() {
                match arg {
                    Expression::Ident(s) => Expression::NEmpty(s.clone()),
                    _ => Expression::FuncCall { name, args },
                }
            } else {
                Expression::FuncCall { name, args }
            }
        }
        "enabled" => {
            if let Some(arg) = args.first() {
                match arg {
                    Expression::Ident(s) => Expression::Enabled(s.clone()),
                    _ => Expression::FuncCall { name, args },
                }
            } else {
                Expression::FuncCall { name, args }
            }
        }
        "np_" => Expression::NP_,
        "pc_value" => {
            if let Some(arg) = args.first() {
                Expression::PcValue(Box::new(arg.clone()))
            } else {
                Expression::FuncCall { name, args }
            }
        }
        "eval" => {
            if let Some(arg) = args.first() {
                Expression::Eval(Box::new(arg.clone()))
            } else {
                Expression::FuncCall { name, args }
            }
        }
        "get_priority" => {
            if let Some(arg) = args.first() {
                Expression::GetPriority(Box::new(arg.clone()))
            } else {
                Expression::FuncCall { name, args }
            }
        }
        "set_priority" => {
            if args.len() >= 2 {
                Expression::SetPriority {
                    pid: Box::new(args[0].clone()),
                    value: Box::new(args[1].clone()),
                }
            } else {
                Expression::FuncCall { name, args }
            }
        }
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
        // Record access: ident.field
        map(
            pair(ident, preceded(ws_char('.'), ident)),
            |(name, field)| Expression::RecordAccess {
                record: Box::new(Expression::Ident(name)),
                field,
            },
        ),
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
