//! Parser helpers: whitespace skipping, keyword/identifier detection.

use nom::{IResult, bytes::complete::tag, bytes::complete::take_while, character::complete::char};

use super::Input;

pub(crate) fn ws_char(c: char) -> impl Fn(Input) -> IResult<Input, char> {
    move |input: Input| {
        let (input, _) = skip_ws(input)?;
        char(c)(input)
    }
}

pub(crate) fn skip_ws(input: Input) -> IResult<Input, ()> {
    let mut pos = 0;
    loop {
        while pos < input.len()
            && input[pos..]
                .chars()
                .next()
                .is_some_and(|c| c.is_whitespace())
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

/// The end of a declaration: a `;`, or the end of the line.
///
/// Promela's own examples write both. `bool b[2]` followed by a comment and `bool client_busy[N]`
/// followed by another declaration are in spin's Examples tree, and requiring the semicolon turned
/// each of them into "N characters left unconsumed at top level" - a parse error on a model spin
/// accepts. A terminator is still required: one of the two, or the declaration is not one.
pub(crate) fn decl_end(input: Input) -> IResult<Input, ()> {
    if let Ok((rest, _)) = symbol(";")(input) {
        return Ok((rest, ()));
    }
    // A declaration immediately before the closing brace of a body: `{ byte q }`.
    if let Ok((rest, _)) = skip_ws(input)
        && rest.starts_with('}')
    {
        return Ok((rest, ()));
    }
    let (rest, _) = take_while(|c: char| c == ' ' || c == '\t' || c == '\r')(input)?;
    match rest.strip_prefix('\n') {
        Some(after) => Ok((after, ())),
        None => Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Char,
        ))),
    }
}
pub(crate) fn symbol(s: &'static str) -> impl Fn(Input) -> IResult<Input, Input> {
    move |input: Input| {
        let (input, _) = skip_ws(input)?;
        tag(s)(input)
    }
}

pub(crate) fn keyword(s: &'static str) -> impl Fn(Input) -> IResult<Input, Input> {
    move |input: Input| {
        let (input, _) = skip_ws(input)?;
        let (input, kw) = tag(s)(input)?;
        if let Some(next) = input.chars().next()
            && (next.is_alphanumeric() || next == '_')
        {
            return Err(nom::Err::Error(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Tag,
            )));
        }
        Ok((input, kw))
    }
}

pub(crate) fn keyword_list() -> Vec<&'static str> {
    vec![
        "active", "assert", "atomic", "break", "byte", "chan", "bool", "d_step", "do", "else",
        "enabled", "empty", "fi", "full", "goto", "hidden", "if", "inline", "int", "len", "mtype",
        "nempty", "never", "nfull", "od", "of", "printf", "proctype", "provided", "run", "short",
        "show", "skip", "timeout", "typedef", "unless", "unsigned", "bit",
    ]
}

pub(crate) fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}
#[allow(dead_code)]
fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

pub(crate) fn ident(input: Input) -> IResult<Input, String> {
    let (input, _) = skip_ws(input)?;
    let (input, raw) = nom::bytes::complete::is_not(" \t\r\n;(){}[]!?:,=+-*/%&|<>^~@")(input)?;
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
