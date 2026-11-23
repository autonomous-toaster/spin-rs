use super::generate;
use crate::parser;

// ── A name is three facts ────────────────────────────────────────
//
// Three defects in this repository were the same shape: the state layout and the emitters agreeing about
// a *name* while disagreeing about its receiver (`s.` vs `state.`), its type (an `init` local emitted
// without one, so `i++` never wrapped), or its existence (an mtype constant read as a field). Each was
// found by a run-time error in one model. This check asks the question of every model at once: every
// state field the generated code reads must be a field the layout declares.

/// Collect the identifiers a chunk indexes off the state table, and the ones the layout declares.
fn state_fields(
    lua: &str,
) -> (
    std::collections::BTreeSet<String>,
    std::collections::BTreeSet<String>,
) {
    let mut declared = std::collections::BTreeSet::new();
    let mut referenced = std::collections::BTreeSet::new();
    let ident = |rest: &str| -> Option<String> {
        let name: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        (!name.is_empty() && !name.chars().next().unwrap().is_ascii_digit()).then_some(name)
    };
    for line in lua.lines() {
        let trimmed = line.trim_start();
        // A declaration is an assignment to a field in the state layout.
        if let Some(rest) = trimmed.strip_prefix("state.")
            && let Some(name) = ident(rest)
            && rest[name.len()..].trim_start().starts_with('=')
        {
            declared.insert(name);
        }
        for prefix in ["s.", "state."] {
            let mut from = 0;
            while let Some(pos) = line[from..].find(prefix) {
                let start = from + pos + prefix.len();
                if let Some(name) = ident(&line[start..]) {
                    referenced.insert(name);
                }
                from = start;
            }
        }
    }
    (declared, referenced)
}

/// Fields the runtime or the emitter provides without a layout declaration.
const PROVIDED_FIELDS: &[&str] = &["_spin_pid", "_failed_assert"];

fn undeclared_state_refs(lua: &str) -> Vec<String> {
    let (declared, referenced) = state_fields(lua);
    referenced
        .into_iter()
        .filter(|name| !declared.contains(name))
        .filter(|name| !PROVIDED_FIELDS.contains(&name.as_str()))
        .collect()
}

#[test]
fn every_state_field_read_is_a_field_declared() {
    // The construct table, because it is the set of shapes most likely to reach an arm nobody tested.
    for c in crate::parity::constructs::CONSTRUCTS {
        let model = match parser::parse(c.source) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let lua = generate(&model).source;
        let bad = undeclared_state_refs(&lua);
        assert!(
            bad.is_empty(),
            "{}: the generated code reads state fields nothing declares: {:?}",
            c.name,
            bad
        );
    }
    // And the generated corpus, which is where a shape no table mentions would appear.
    for m in crate::parity::corpus() {
        let model = match parser::parse(&m.source) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let lua = generate(&model).source;
        let bad = undeclared_state_refs(&lua);
        assert!(
            bad.is_empty(),
            "{}: the generated code reads state fields nothing declares: {:?}",
            m.name,
            bad
        );
    }
}

#[test]
fn the_check_can_fail() {
    // A control, because a check that passes on every model in the tree says nothing until it is shown
    // to fail on the shape it was written for: this is the exact line the layout emitted for
    // `mtype x = a` before the receiver was made a property of the emission context.
    let buggy = "function _spin_init_state()\n    local state = {}\n    state.P_x = s.a\n    return state\nend\n";
    assert_eq!(undeclared_state_refs(buggy), vec!["a".to_string()]);
    // And a line with the receiver right must not be reported.
    let fine = "function _spin_init_state()\n    local state = {}\n    state.a = 3\n    state.P_x = state.a\n    return state\nend\n";
    assert!(undeclared_state_refs(fine).is_empty());
}
