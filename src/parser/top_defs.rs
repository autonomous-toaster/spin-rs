//! Top-level definition parsers.

use nom::{
    branch::alt,
    character::complete::char,
    combinator::{map, opt},
    multi::{many0, separated_list0, separated_list1},
    sequence::{delimited, pair, preceded},
    IResult,
};

use super::Input;
use super::*;

pub(crate) fn proctype_def(input: Input) -> IResult<Input, TopLevel> {
    let (input, _) = skip_ws(input)?;
    let (input, active_kw) = opt(keyword("active"))(input)?;
    // Handle optional active count: active [N] proctype
    let (input, active_count) = if active_kw.is_some() {
        opt(delimited(ws_char('['), int_literal, ws_char(']')))(input)?
    } else {
        (input, None)
    };
    let (input, _) = keyword("proctype")(input)?;
    let (input, name) = ident(input)?;
    let (input, params) = delimited(
        ws_char('('),
        opt(separated_list0(symbol(","), var_decl)),
        ws_char(')'),
    )(input)?;
    let (input, body) = delimited(ws_char('{'), many0(stmt), ws_char('}'))(input)?;
    let active = active_kw.is_some();
    let pid = active_count;
    Ok((
        input,
        TopLevel::Proctype(ProctypeDef {
            name,
            active,
            provided: None,
            parameters: params.unwrap_or_default(),
            body,
            pid,
            line: 0,
        }),
    ))
}
pub(crate) fn init_def(input: Input) -> IResult<Input, TopLevel> {
    let (input, _) = keyword("init")(input)?;
    let (input, body) = delimited(ws_char('{'), many0(stmt), ws_char('}'))(input)?;
    Ok((input, TopLevel::Init(InitDef { body, line: 0 })))
}
pub(crate) fn never_claim(input: Input) -> IResult<Input, TopLevel> {
    let (input, _) = keyword("never")(input)?;
    let (input, body) = delimited(ws_char('{'), many0(stmt), ws_char('}'))(input)?;
    Ok((input, TopLevel::NeverClaim(NeverClaim { body, line: 0 })))
}
pub(crate) fn ltl_formula(input: Input) -> IResult<Input, TopLevel> {
    let (input, _) = keyword("ltl")(input)?;
    let (input, name) = opt(ident)(input)?;
    let (input, _) = ws_char('{')(input)?;
    let (input, formula) = nom::bytes::complete::take_while(|c: char| c != '}')(input)?;
    let (input, _) = ws_char('}')(input)?;
    Ok((
        input,
        TopLevel::Ltl(LtlFormula {
            name,
            formula: formula.trim().to_string(),
            line: 0,
        }),
    ))
}

pub(crate) fn inline_def(input: Input) -> IResult<Input, TopLevel> {
    let (input, _) = keyword("inline")(input)?;
    let (input, name) = ident(input)?;
    let (input, params) = delimited(
        ws_char('('),
        separated_list0(symbol(","), ident),
        ws_char(')'),
    )(input)?;
    let (input, body) = delimited(ws_char('{'), many0(stmt), ws_char('}'))(input)?;
    Ok((
        input,
        TopLevel::Inline(InlineDef {
            name,
            parameters: params,
            body,
            line: 0,
        }),
    ))
}

pub(crate) fn preprocessor(input: Input) -> IResult<Input, TopLevel> {
    let (input, _) = skip_ws(input)?;
    if !input.starts_with('#') {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Tag,
        )));
    }
    let (input, content) = nom::bytes::complete::take_while(|c: char| c != '\n')(&input[1..])?;
    let (input, _) = opt(char('\n'))(input)?;
    Ok((
        input,
        TopLevel::PreprocessorDirective(format!("#{}", content)),
    ))
}
pub(crate) fn c_code_block(input: Input) -> IResult<Input, TopLevel> {
    let (input, _) = keyword("c_code")(input)?;
    let (input, _) = ws_char('{')(input)?;
    let (input, code) = nom::bytes::complete::take_while(|c: char| c != '}')(input)?;
    let (input, _) = ws_char('}')(input)?;
    Ok((input, TopLevel::CCode(code.trim().to_string(), 0)))
}
/// Parse a channel array declaration: `chan name[N];`
pub(crate) fn chan_array_decl(input: Input) -> IResult<Input, TopLevel> {
    let (input, _) = keyword("chan")(input)?;
    let (input, name) = ident(input)?;
    let (input, _) = ws_char('[')(input)?;
    let (input, size) = delimited(skip_ws, int_literal, skip_ws)(input)?;
    let (input, _) = ws_char(']')(input)?;
    let (input, _) = symbol(";")(input)?;
    Ok((
        input,
        TopLevel::ChannelArray {
            name: name.to_string(),
            size,
            line: 0,
        },
    ))
}

/// Parse a channel declaration: `chan name = [N] of { type };`
pub(crate) fn chan_decl(input: Input) -> IResult<Input, TopLevel> {
    let (input, _) = keyword("chan")(input)?;
    let (input, name) = ident(input)?;
    let (input, _) = symbol("=")(input)?;
    let (input, _) = ws_char('[')(input)?;
    let (input, capacity) = delimited(skip_ws, int_literal, skip_ws)(input)?;
    let (input, _) = ws_char(']')(input)?;
    let (input, _) = keyword("of")(input)?;
    let (input, _) = ws_char('{')(input)?;
    let (input, _msg_type) = var_type(input)?;
    let (input, _) = ws_char('}')(input)?;
    let (input, _) = symbol(";")(input)?;
    Ok((
        input,
        TopLevel::ChanDecl {
            name: name.to_string(),
            capacity,
            line: 0,
        },
    ))
}

pub(crate) fn top_level(input: Input) -> IResult<Input, Vec<TopLevel>> {
    let (input, _) = skip_ws(input)?;
    alt::<_, _, nom::error::Error<Input>, _>((
        map(proctype_def, |p| vec![p]),
        map(init_def, |i| vec![i]),
        map(never_claim, |n| vec![n]),
        map(ltl_formula, |l| vec![l]),
        map(inline_def, |i| vec![i]),
        map(preprocessor, |p| vec![p]),
        map(c_code_block, |c| vec![c]),
        map(chan_decl, |c| vec![c]),
        map(chan_array_decl, |ca| vec![ca]),
        map(var_decl_list, |vds| {
            vds.into_iter().map(TopLevel::GlobalVar).collect()
        }),
    ))(input)
}

/// Parse a list of comma-separated variable declarations: `type name1, name2 = init;`
pub(crate) fn var_decl_list(input: Input) -> IResult<Input, Vec<VarDecl>> {
    let (input, vt) = var_type(input)?;
    let (input, decls) = separated_list1(
        symbol(","),
        map(
            pair(
                ident,
                opt(pair(opt(array_dim), opt(preceded(symbol("="), expr)))),
            ),
            |(name, rest)| {
                let (arr, init) = rest.unwrap_or((None, None));
                VarDecl {
                    var_type: vt.clone(),
                    name,
                    array_size: arr,
                    init: init.map(Box::new),
                    line: 0,
                }
            },
        ),
    )(input)?;
    let (input, _) = symbol(";")(input)?;
    Ok((input, decls))
}
