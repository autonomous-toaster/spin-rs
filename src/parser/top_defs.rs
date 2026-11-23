//! Top-level definition parsers.

use nom::{
    IResult,
    branch::alt,
    character::complete::char,
    combinator::{map, opt},
    multi::{many0, separated_list0, separated_list1},
    sequence::{delimited, pair, preceded},
};

use super::Input;
use super::*;

/// A proctype parameter list entry, which may declare several names with one type.
///
/// Promela writes `(short id, a, b)` for three shorts and `(short id, byte x)` for two parameters,
/// so a comma continues the name list only while the next word is not a type. A qualified type
/// (`mtype:fruit z`) is the third form.
fn param_decls(input: Input) -> IResult<Input, Vec<VarDecl>> {
    let (input, var_type) = var_type(input)?;
    let (input, _) = opt(|i| {
        let (i, _) = ws_char(':')(i)?;
        ident(i)
    })(input)?;
    let (mut input, first) = ident(input)?;
    let mut out = vec![VarDecl {
        var_type: var_type.clone(),
        name: first,
        array_size: None,
        init: None,
        line: 0,
    }];
    loop {
        let peek = match symbol(",")(input) {
            Ok((rest, _)) => rest,
            Err(_) => break,
        };
        // A comma followed by a type keyword starts the next parameter instead.
        let next_word = peek
            .trim_start()
            .split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .next()
            .unwrap_or("")
            .to_string();
        const TYPE_WORDS: &[&str] = &[
            "bit", "bool", "byte", "short", "int", "unsigned", "chan", "mtype", "hidden",
        ];
        if TYPE_WORDS.contains(&next_word.as_str()) {
            // Leave the comma for the caller: it separates two parameters, not two names.
            break;
        }
        match ident(peek) {
            Ok((rest, name)) => {
                out.push(VarDecl {
                    var_type: var_type.clone(),
                    name,
                    array_size: None,
                    init: None,
                    line: 0,
                });
                input = rest;
            }
            Err(_) => break,
        }
    }
    Ok((input, out))
}

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
        opt(separated_list1(
            alt((symbol(","), symbol(";"))),
            param_decls,
        )),
        ws_char(')'),
    )(input)?;
    // `provided (expr)` and `priority N` come between the parameter list and the body.
    let (input, provided) = opt(|i| {
        let (i, _) = keyword("provided")(i)?;
        let (i, _) = ws_char('(')(i)?;
        let (i, e) = expr(i)?;
        let (i, _) = ws_char(')')(i)?;
        Ok((i, e))
    })(input)?;
    let (input, priority) = opt(|i| {
        let (i, _) = keyword("priority")(i)?;
        let (i, n) = int_literal(i)?;
        Ok((i, n))
    })(input)?;
    let (input, body) = delimited(ws_char('{'), many0(stmt), ws_char('}'))(input)?;
    let active = active_kw.is_some();
    let pid = active_count;
    Ok((
        input,
        TopLevel::Proctype(ProctypeDef {
            name,
            active,
            provided,
            priority,
            parameters: params.unwrap_or_default().into_iter().flatten().collect(),
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
    // spin accepts a terminating semicolon, and models in the wild carry it.
    let (input, _) = opt(symbol(";"))(input)?;
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
/// Parse a channel array declaration: `chan name[N] = [cap] of { type };`
///
/// The capacity and message type are optional here because `chan name[N];` also appears; without
/// them the elements are registered as rendezvous channels, which is what the type-less form means.
pub(crate) fn chan_array_decl(input: Input) -> IResult<Input, TopLevel> {
    let (input, _) = keyword("chan")(input)?;
    let (input, name) = ident(input)?;
    let (input, _) = ws_char('[')(input)?;
    let (input, size) = delimited(skip_ws, int_literal, skip_ws)(input)?;
    let (input, _) = ws_char(']')(input)?;
    let (input, rest) = opt(|i| {
        let (i, _) = symbol("=")(i)?;
        let (i, _) = ws_char('[')(i)?;
        let (i, cap) = delimited(skip_ws, int_literal, skip_ws)(i)?;
        let (i, _) = ws_char(']')(i)?;
        let (i, _) = keyword("of")(i)?;
        let (i, _) = ws_char('{')(i)?;
        let (i, types) = separated_list1(symbol(","), message_field)(i)?;
        let (i, _) = ws_char('}')(i)?;
        Ok((i, (cap, types.len())))
    })(input)?;
    let (input, _) = opt(symbol(";"))(input)?;
    Ok((
        input,
        TopLevel::ChannelArray {
            name: name.to_string(),
            size,
            capacity: rest.map(|(cap, _)| cap).unwrap_or(0),
            fields: rest.map(|(_, n)| n).unwrap_or(1),
            line: 0,
        },
    ))
}

/// One field type inside a channel's message list: `int`, `mtype`, or a qualified `mtype:fruit`.
fn message_field(input: Input) -> IResult<Input, ()> {
    let (input, _) = var_type(input)?;
    let (input, _) = opt(|i| {
        let (i, _) = ws_char(':')(i)?;
        ident(i)
    })(input)?;
    Ok((input, ()))
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
    // A message may carry several typed fields, and a field type may be a named mtype. The types
    // are not stored - a message is a list of values here - but the *count* is, because pan rejects
    // a send or a receive whose arity does not match the declaration and a model that does it has
    // no verdict to agree with.
    let (input, fields) = separated_list1(symbol(","), message_field)(input)?;
    let (input, _) = ws_char('}')(input)?;
    // The terminator is optional in Promela, and spin's own examples omit it.
    let (input, _) = opt(symbol(";"))(input)?;
    Ok((
        input,
        TopLevel::ChanDecl {
            name: name.to_string(),
            capacity,
            fields: fields.len(),
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
        map(mtype_decl, |m| vec![m]),
        map(typedef_decl, |t| vec![t]),
        map(var_decl_list, |vds| {
            vds.into_iter().map(TopLevel::GlobalVar).collect()
        }),
    ))(input)
}

/// Parse mtype declaration: `mtype = { name1, name2, ... }`
pub(crate) fn mtype_decl(input: Input) -> IResult<Input, TopLevel> {
    let (input, _) = keyword("mtype")(input)?;
    // Promela writes `mtype = { a, b }`, the older `mtype { a, b }`, and a named form
    // `mtype:fruit = { appel, peer }`.
    let (input, _) = opt(|i| {
        let (i, _) = ws_char(':')(i)?;
        ident(i)
    })(input)?;
    let (input, _) = opt(symbol("="))(input)?;
    let (input, _) = ws_char('{')(input)?;
    let (input, names) = separated_list1(symbol(","), ident)(input)?;
    let (input, _) = ws_char('}')(input)?;
    let (input, _) = opt(symbol(";"))(input)?;
    Ok((input, TopLevel::MtypeDecl { names, line: 0 }))
}

/// Parse typedef declaration: `typedef MyStruct { byte a; int b }`
pub(crate) fn typedef_decl(input: Input) -> IResult<Input, TopLevel> {
    let (input, _) = keyword("typedef")(input)?;
    let (input, name) = ident(input)?;
    let (input, _) = ws_char('{')(input)?;
    // A struct's fields are declarations terminated by semicolons; `var_decl` does not consume the
    // terminator itself, so without this only a single field would parse.
    let (input, fields) = many0(nom::sequence::terminated(var_decl, opt(symbol(";"))))(input)?;
    let (input, _) = ws_char('}')(input)?;
    let (input, _) = opt(symbol(";"))(input)?;
    Ok((
        input,
        TopLevel::Typedef {
            name,
            fields,
            line: 0,
        },
    ))
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
