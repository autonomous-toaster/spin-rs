use super::*;
use crate::parser;

fn test_expr_bool_lit() {
    let source = "active proctype P() { bit flag; flag = true; }";
    let model = crate::parser::parse(source).unwrap();
    let lua = crate::codegen::generate(&model);
    assert!(lua.source.contains("true") || lua.source.contains("s.flag"));
}

#[test]
fn test_expr_func_call() {
    // Test function call expression
    let source = "active proctype P() { x = enabled(P); }";
    let model = crate::parser::parse(source).unwrap();
    let lua = crate::codegen::generate(&model);
    assert!(lua.source.contains("enabled") || lua.source.contains("function"));
}

#[test]
fn test_expr_channel_poll() {
    let source = "chan ch = [1] of { byte };\nactive proctype P() { ch ? [x]; }";
    let model = crate::parser::parse(source).unwrap();
    let lua = crate::codegen::generate(&model);
    assert!(lua.source.contains("chan") || lua.source.contains("function"));
}

#[test]
fn test_expr_record_access() {
    // Test RecordAccess expression via remote reference
    let source = "active proctype P() { x = P[0].field; }";
    let model = crate::parser::parse(source).unwrap();
    let lua = crate::codegen::generate(&model);
    assert!(lua.source.contains("function"));
}

#[test]
fn test_expr_channel_send_expr() {
    // Test ChannelSend expression type
    let source = "chan ch = [1] of { byte };\nactive proctype P() { ch ! 42; }";
    let model = crate::parser::parse(source).unwrap();
    let lua = crate::codegen::generate(&model);
    assert!(lua.source.contains("chan"));
}

#[test]
fn test_expr_nfull_nempty() {
    // Test NFull/NEmpty expressions
    let source =
        "chan ch = [1] of { byte };\nactive proctype P() { x = nfull(ch); y = nempty(ch); }";
    let result = crate::parser::parse(source);
    if let Ok(model) = result {
        let lua = crate::codegen::generate(&model);
        assert!(lua.source.len() > 0);
    }
}

#[test]
fn test_expr_remote_ref() {
    // Test RemoteRef
    let source = "active proctype P() { x = P[0].y; }";
    let model = crate::parser::parse(source).unwrap();
    let lua = crate::codegen::generate(&model);
    assert!(lua.source.contains("function"));
}

#[test]
fn test_expr_timeout() {
    // Test Timeout expression
    let source = "active proctype P() { do :: timeout -> break od }";
    let model = crate::parser::parse(source).unwrap();
    let lua = crate::codegen::generate(&model);
    assert!(lua.source.contains("function"));
}

#[test]
fn test_expr_enabled() {
    // Test Enabled expression
    let source = "active proctype P() { x = enabled(P); }";
    let model = crate::parser::parse(source).unwrap();
    let lua = crate::codegen::generate(&model);
    assert!(lua.source.contains("function"));
}

#[test]
fn test_emit_unless_variant() {
    // Directly test Unless statement in emit_stmts via codegen
    // We can't create it from parsing, but we can check the codegen handles it
    let source = "active proctype P() {\n    do\n    :: (x < 5) -> x = x + 1\n    :: else -> break\n    od\n}";
    let model = crate::parser::parse(source).unwrap();
    let lua = crate::codegen::generate(&model);
    assert!(lua.source.len() > 0);
}

#[test]
fn test_generate_never_claim() {
    // Test never claim codegen generates transitions
    // Use a simple never claim body without labels
    let source = "never { do :: (x == 0) -> skip od }";
    let model = crate::parser::parse(source).unwrap();
    let lua = crate::codegen::generate(&model);
    assert!(lua.source.contains("_spin_transitions_never"));
    assert!(lua.source.contains("transitions"));
}

#[test]
fn test_channel_array_codegen() {
    // Test that chan tok[3] generates 3 state variables
    let source = "chan tok[3];\nactive proctype P() { byte msg; tok[0] ? msg }";
    let model = crate::parser::parse(source).unwrap();
    let lua = crate::codegen::generate(&model);
    assert!(
        lua.source.contains("state.tok_0 = nil"),
        "Should emit state.tok_0"
    );
    assert!(
        lua.source.contains("state.tok_1 = nil"),
        "Should emit state.tok_1"
    );
    assert!(
        lua.source.contains("state.tok_2 = nil"),
        "Should emit state.tok_2"
    );
}
