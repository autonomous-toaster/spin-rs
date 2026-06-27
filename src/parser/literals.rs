//! Literal parsers: integers, strings, variable types.

use nom::{
    IResult,
    branch::alt,
    bytes::complete::tag,
    character::complete::char,
    combinator::{map, opt, value},
    sequence::{delimited, preceded},
};

use super::Input;
use super::ast::*;
use super::helpers::*;

// ─── Literals ───────────────────────────────────────────────────
pub(crate) fn int_literal(input: Input) -> IResult<Input, i64> {
    let (input, _) = skip_ws(input)?;
    let (input, sign) = opt(tag("-"))(input)?;
    let (input, digits) = nom::bytes::complete::take_while1(|c: char| c.is_ascii_digit())(input)?;
    let val: i64 = digits.parse().unwrap_or(0);
    Ok((input, if sign.is_some() { -val } else { val }))
}

pub(crate) fn string_literal(input: Input) -> IResult<Input, String> {
    let (input, _) = skip_ws(input)?;
    let (input, s) = delimited(
        char('"'),
        nom::bytes::complete::take_while(|c: char| c != '"'),
        char('"'),
    )(input)?;
    Ok((input, s.to_string()))
}

pub(crate) fn var_type(input: Input) -> IResult<Input, VarType> {
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
