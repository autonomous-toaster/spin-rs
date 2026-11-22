use super::*;
use crate::parser;

#[test]
fn test_generate_basic_proctype() {
    let source = "active proctype P() { byte x; x = 1; }";
    let model = parser::parse(source).unwrap();
    let lua = generate(&model);
    assert!(lua.source.contains("function _spin_init_state"));
    assert!(lua.source.contains("function _spin_transitions_P"));
    assert!(lua.source.contains("function _spin_get_transitions"));
}

#[test]
fn test_generate_with_guards() {
    let source = "active proctype P() { if :: (x > 0) -> y = 1 :: else -> y = 0 fi }";
    let model = parser::parse(source).unwrap();
    let lua = generate(&model);
    assert!(lua.source.contains("guard"));
    assert!(lua.source.contains("effect"));
}

#[test]
fn test_generate_ltl() {
    let source = "ltl p0 { [](x == 0) }\nactive proctype P() { x = 1; }";
    let model = parser::parse(source).unwrap();
    let lua = generate(&model);
    assert!(lua.source.contains("LTL: p0"));
}

#[test]
fn test_generate_goto_break() {
    // Use if/fi with goto-like constructs
    let source = "active proctype P() {\n    do\n    :: (x > 0) -> x = x - 1\n    :: else -> break\n    od\n}";
    let model = parser::parse(source).unwrap();
    let lua = generate(&model);
    assert!(lua.source.contains("_done_P"));
}

#[test]
fn test_generate_channel_send_recv() {
    let source = "chan ch = [1] of { byte };\nactive proctype P() { ch ! 42; ch ? x; }";
    let model = parser::parse(source).unwrap();
    let lua = generate(&model);
    assert!(lua.source.contains("chan_send") || lua.source.contains("!"));
    assert!(lua.source.contains("chan_recv") || lua.source.contains("?"));
}

#[test]
fn test_generate_printf() {
    let source = "active proctype P() { printf(\"x = %d\", x); }";
    let model = parser::parse(source).unwrap();
    let lua = generate(&model);
    assert!(lua.source.contains("printf"));
}

#[test]
fn test_generate_assert() {
    let source = "active proctype P() { assert(x > 0); }";
    let model = parser::parse(source).unwrap();
    let lua = generate(&model);
    assert!(lua.source.contains("assert"));
}

#[test]
fn test_generate_run() {
    let source = "proctype Q() { byte y; y = 1; }\nactive proctype P() { run Q(); }";
    let model = parser::parse(source).unwrap();
    let lua = generate(&model);
    assert!(lua.source.contains("run"));
}

#[test]
fn test_generate_atomic() {
    // Simple assignment
    let source = "active proctype P() { x = 1; y = 2 }";
    let model = parser::parse(source).unwrap();
    let lua = generate(&model);
    // Just verify we can parse and generate
    assert!(lua.source.contains("function"));
}

#[test]
#[ignore = "pre-existing parser gap: len()/empty()/full() do not parse; this test previously passed only because parse() silently truncated the model"]
fn test_generate_special_expr() {
    let source = "active proctype P() {\n    byte arr[3];\n    int x;\n    x = len(arr);\n    x = empty(arr);\n    x = full(arr);\n}";
    let model = parser::parse(source).unwrap();
    let lua = generate(&model);
    assert!(
        lua.source.contains("len") || lua.source.contains("empty") || lua.source.contains("full")
    );
}

#[test]
fn test_generate_unless() {
    let source = "active proctype P() {\n    do\n    :: (x < 10) -> x = x + 1\n    :: else -> break\n    od\n}";
    let model = parser::parse(source).unwrap();
    let lua = generate(&model);
    // The generated Lua should have structured output
    assert!(lua.source.is_empty() == false);
}

#[test]
fn test_generate_array_access() {
    let source = "active proctype P() { byte arr[3]; arr[0] = 42; }";
    let model = parser::parse(source).unwrap();
    let lua = generate(&model);
    assert!(lua.source.contains("arr"));
}

#[test]
fn test_generate_nested_if() {
    let source = "active proctype P() {\n    if\n    :: (x > 0) ->\n        if\n        :: (x > 5) -> y = 1\n        :: else -> y = 0\n        fi\n    :: else -> y = -1\n    fi\n}";
    let model = parser::parse(source).unwrap();
    let lua = generate(&model);
    assert!(lua.source.contains("guard"));
}

#[test]
fn test_generate_guarded_goto() {
    // Test that guard bodies with different statement types generate properly
    let source = "active proctype P() {\n    do\n    :: (x < 10) -> assert(x >= 0); x = x + 1\n    :: (x >= 10) -> break\n    od\n}";
    let model = parser::parse(source).unwrap();
    let lua = generate(&model);
    assert!(lua.source.contains("assert"));
    assert!(lua.source.contains("done"));
}

#[test]
fn test_generate_unless_construct() {
    // Test unless-like construct using do/od (actual unless is not parsed yet)
    let source = "active proctype P() {\n    do\n    :: (x < 5) -> x = x + 1\n    :: (x >= 5) -> skip\n    od\n}";
    let model = parser::parse(source).unwrap();
    let lua = generate(&model);
    assert!(lua.source.contains("skip") || lua.source.contains("guard"));
}

#[test]
fn test_generate_expr_stmt() {
    // Expression statement (bare expression)
    let source = "active proctype P() { wait(x > 0); }";
    let model = parser::parse(source).unwrap();
    let lua = generate(&model);
    assert!(lua.source.contains("expression") || lua.source.contains("function"));
}

#[test]
#[ignore = "pre-existing parser gap: `ch ? x` in a guard does not parse; this test previously passed only because parse() silently truncated the model"]
fn test_generate_recv_in_guard() {
    // Test send/recv inside guard to reach those emit_stmts branches
    let source = "chan ch = [1] of { byte };\nactive proctype P() {\n    do\n    :: ch ? x -> skip\n    od\n}";
    let model = parser::parse(source).unwrap();
    let lua = generate(&model);
    assert!(lua.source.contains("recv") || lua.source.contains("chan"));
}

#[test]
fn test_generate_goto_label() {
    // Goto within a single proctype produces correct state sequence
    let source = "active proctype P() {\n    goto target;\n    x = 1;\n    target: x = 2;\n}";
    let model = parser::parse(source).unwrap();
    let lua = generate(&model);
    // A goto inside a body of more than one statement is refused, not emitted: the previous
    // encoding set _done_P = true in order to jump, which killed the process and left the label
    // unreachable. Emitting that was a silent wrong, so the step machine refuses until goto has an
    // edge target (full-parity 2.5).
    assert!(
        lua.source
            .contains("goto is not supported as a step inside a sequence"),
        "goto should be refused inside a sequence: {}",
        lua.source
    );
    // The program counter the refusal will use still exists.
    assert!(lua.source.contains("_pc_P"));
}

#[test]
fn test_generate_break_in_do() {
    // Break exits do-loop correctly
    let source = "active proctype P() {\n    do\n    :: (x > 0) -> x = x - 1\n    :: else -> break\n    od\n    y = 1;\n}";
    let model = parser::parse(source).unwrap();
    let lua = generate(&model);
    // Should have _done_P for break (break inside do guard sets _done_P)
    assert!(lua.source.contains("_done_P"));
    // Should have step-counter for do-loop body
    assert!(lua.source.contains("_step_P"));
}

#[test]
fn test_generate_standalone_break() {
    // Standalone break statement (not inside guard body)
    let source = "active proctype P() {\n    break;\n}";
    let model = parser::parse(source).unwrap();
    let lua = generate(&model);
    // Should have break transition with _pc
    assert!(lua.source.contains("break:exit"));
}

#[test]
fn test_generate_label_reachable_but_refused() {
    // A label as a goto target is refused inside a sequence rather than emitted with the old
    // encoding, which jumped by marking the process done (see full-parity 2.5).
    let source = "active proctype P() {\n    goto start;\n    start: x = 1;\n}";
    let model = parser::parse(source).unwrap();
    let lua = generate(&model);
    assert!(
        lua.source
            .contains("is not supported as a step inside a sequence"),
        "a label in a sequence should be refused: {}",
        lua.source
    );
}

// ─── T1.3: dependency metadata must cover everything a transition touches ───────

/// Parse the declared metadata and the actual `s.<field>` references out of a generated
/// transition, and assert the declared sets cover the actual ones.
///
/// Only the covering direction is asserted: declaring more than a transition touches is safe
/// (it costs reduction), while declaring less would claim independence from a transition it
/// actually conflicts with, which is unsound. Opaque transitions declare nothing and are exempt,
/// because "unknown" is already the conservative answer.
#[test]
fn test_transition_dependencies_cover_effects() {
    fn field_set(s: &str) -> std::collections::BTreeSet<String> {
        let mut out = std::collections::BTreeSet::new();
        let bytes = s.as_bytes();
        let mut i = 0;
        while i + 2 < bytes.len() {
            if bytes[i] == b's' && bytes[i + 1] == b'.' {
                let start = i + 2;
                let mut j = start;
                while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                    j += 1;
                }
                if j > start {
                    out.insert(s[start..j].to_string());
                }
                i = j;
            } else {
                i += 1;
            }
        }
        out
    }

    fn declared(s: &str, key: &str) -> std::collections::BTreeSet<String> {
        let pat = format!("{} = {{", key);
        let Some(p) = s.find(&pat) else {
            return Default::default();
        };
        let rest = &s[p + pat.len()..];
        let Some(end) = rest.find('}') else {
            return Default::default();
        };
        rest[..end]
            .split(',')
            .map(|v| v.trim().trim_matches('"').to_string())
            .filter(|v| !v.is_empty())
            .collect()
    }

    let models = [
        "bool g = 0;\nactive proctype P() { g = 1; }\nltl p0 { [] ( !(g) ) }",
        "byte x = 0;\nactive proctype P() { x = 1; assert(x == 1); }",
        "bool f = 0;\nactive proctype P() { do :: (1) -> if :: f = 1; :: skip; fi; break od }",
        "chan ch = [1] of { byte };\nactive proctype P() { ch ! 1 }\nactive proctype Q() { byte v; ch ? v }",
    ];

    let mut checked = 0usize;
    for src in models {
        let ast = crate::parser::parse(src).unwrap();
        let lua = crate::codegen::generate(&ast);
        // Split on transition starts; the generated subset has no nested table literals except
        // inside the transition table itself.
        let mut rest = lua.source.as_str();
        while let Some(p) = rest.find("table.insert(transitions, {") {
            let after = &rest[p..];
            let Some(end) = after.find("})") else { break };
            let block = &after[..end];
            rest = &after[end..];

            assert!(
                block.contains("owner = "),
                "every transition must declare an owner: {block}"
            );
            let opaque = block.contains("opaque = true");
            if opaque {
                continue;
            }
            let mut covered = declared(block, "reads");
            covered.extend(declared(block, "writes"));
            let actual = field_set(block);
            for f in &actual {
                // _pid is engine bookkeeping the emitter sets outside the table.
                if f == "_pid" {
                    continue;
                }
                assert!(
                    covered.contains(f),
                    "transition touches `{}` but does not declare it. declared={:?} block:\n{}",
                    f,
                    covered,
                    block
                );
            }
            checked += 1;
        }
    }
    assert!(checked > 0, "the scan found no transitions to verify");
}
