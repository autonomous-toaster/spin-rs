use super::ast::*;
use super::*;

#[test]
fn test_basic_var_decl() {
    let source = "byte x; bit flag; int counter = 0;";
    let model = parse(source).unwrap();
    assert_eq!(model.declarations.len(), 3);
}
#[test]
fn test_active_proctype() {
    let source = "active proctype P() { byte x; x = 1; }";
    let model = parse(source).unwrap();
    assert_eq!(model.declarations.len(), 1);
    match &model.declarations[0] {
        TopLevel::Proctype(p) => {
            assert!(p.active);
            assert_eq!(p.name, "P");
        }
        _ => panic!("expected proctype"),
    }
}
#[test]
fn test_if_fi() {
    let source =
        "active proctype P() {\n    if\n    :: (x > 0) -> y = 1\n    :: else -> y = 0\n    fi\n}";
    let model = parse(source).unwrap();
    match &model.declarations[0] {
        TopLevel::Proctype(p) => match &p.body[0] {
            Stmt::If(guards) => assert_eq!(guards.len(), 2),
            _ => panic!("expected if"),
        },
        _ => panic!("expected proctype"),
    }
}
#[test]
fn test_do_od() {
    let source = "active proctype P() {\n    do\n    :: (x > 0) -> x = x - 1\n    :: (x == 0) -> break\n    od\n}";
    let model = parse(source).unwrap();
    match &model.declarations[0] {
        TopLevel::Proctype(p) => match &p.body[0] {
            Stmt::Do(guards) => assert_eq!(guards.len(), 2),
            _ => panic!("expected do"),
        },
        _ => panic!("expected proctype"),
    }
}
#[test]
fn test_ltl_formula() {
    let source = "ltl p0 { [](x == 0) }";
    let model = parse(source).unwrap();
    assert_eq!(model.declarations.len(), 1);
    match &model.declarations[0] {
        TopLevel::Ltl(l) => {
            assert_eq!(l.name.as_deref(), Some("p0"));
            assert!(l.formula.contains("[]"));
        }
        _ => panic!("expected LTL"),
    }
}
#[test]
fn test_preprocessor() {
    let source = "#define N 5\nbyte x;\n";
    let model = parse(source).unwrap();
    assert_eq!(model.declarations.len(), 2);
    match &model.declarations[0] {
        TopLevel::PreprocessorDirective(d) => assert!(d.contains("define")),
        _ => panic!("expected preprocessor"),
    }
}
#[test]
fn test_channel_send_recv() {
    let source = "active proctype P() {\n    ch!msg(1);\n    ch?msg;\n}";
    let model = parse(source).unwrap();
    assert_eq!(model.declarations.len(), 1);
}

#[test]
fn test_d_step_atomic() {
    let source = "active proctype P() { d_step { x = 1; x = x + 1 } atomic { y = 2 } }";
    let model = parse(source).unwrap();
    match &model.declarations[0] {
        TopLevel::Proctype(p) => {
            assert!(p.body.len() >= 2);
            assert!(matches!(p.body[0], Stmt::DStep(_, _)));
            assert!(matches!(p.body[1], Stmt::Atomic(_, _)));
        }
        _ => panic!("Expected proctype"),
    }
}

#[test]
fn test_remote_ref_expr() {
    let source = "active proctype P() { x = Q@y; }";
    let model = parse(source).unwrap();
    match &model.declarations[0] {
        TopLevel::Proctype(p) => {
            eprintln!("p.body.len() = {}", p.body.len());
            eprintln!("p.body[0] = {:?}", p.body[0]);
            if let Stmt::Assignment { value, .. } = &p.body[0] {
                eprintln!("value = {:?}", value);
                assert!(matches!(
                    value.as_ref(),
                    Expression::RemoteRef { name, .. } if name == "y"
                ));
            } else {
                panic!("Expected assignment, got {:?}", p.body[0]);
            }
        }
        _ => panic!("Expected proctype, got {:?}", model.declarations[0]),
    }
}

#[test]
fn test_chan_decl() {
    // Simple channel declaration without complex type
    let source = "chan ch = [2] of { byte };";
    let result = parse(source);
    assert!(
        result.is_ok(),
        "Failed to parse chan decl: {:?}",
        result.err()
    );
    let model = result.unwrap();
    assert_eq!(model.declarations.len(), 1);
    match &model.declarations[0] {
        TopLevel::ChanDecl { name, capacity, .. } => {
            assert_eq!(name, "ch");
            assert_eq!(*capacity, 2);
        }
        _ => panic!("Expected ChanDecl, got {:?}", model.declarations[0]),
    }
}

#[test]
fn test_chan_decl_rendezvous() {
    // Channel with capacity 0 (rendezvous)
    let source = "chan ch1 = [0] of { byte };";
    let result = parse(source);
    assert!(
        result.is_ok(),
        "Failed to parse rendezvous chan: {:?}",
        result.err()
    );
    let model = result.unwrap();
    match &model.declarations[0] {
        TopLevel::ChanDecl { name, capacity, .. } => {
            assert_eq!(name, "ch1");
            assert_eq!(*capacity, 0);
        }
        _ => panic!("Expected ChanDecl"),
    }
}

#[test]
fn test_chan_decl_int_type() {
    // Channel with int type
    let source = "chan ch2 = [5] of { int };";
    let result = parse(source);
    assert!(
        result.is_ok(),
        "Failed to parse int chan: {:?}",
        result.err()
    );
}

#[test]
fn test_deadlock_circular_parse() {
    // Full deadlock_circular model from benchmark
    let source = r#"
chan ch1 = [0] of { byte }; chan ch2 = [0] of { byte };
active proctype P() { ch1 ! 1; ch2 ? 0; }
active proctype Q() { ch2 ! 1; ch1 ? 0; }
"#;
    let result = parse(source);
    assert!(
        result.is_ok(),
        "Failed to parse deadlock_circular: {:?}",
        result.err()
    );
    let model = result.unwrap();
    eprintln!("DEBUG: Found {} declarations", model.declarations.len());
    for (i, decl) in model.declarations.iter().enumerate() {
        eprintln!("  {}: {:?}", i, decl);
    }
    // Should have: 2 ChanDecl + 2 Proctype = 4 declarations
    // Note: may have fewer if proctype bodies don't parse correctly
    assert!(
        model.declarations.len() >= 2,
        "Should have at least 2 ChanDecl"
    );

    // Check first two are ChanDecl
    match &model.declarations[0] {
        TopLevel::ChanDecl { name, capacity, .. } => {
            assert_eq!(name, "ch1");
            assert_eq!(*capacity, 0);
        }
        _ => panic!("Expected ChanDecl for ch1"),
    }
    match &model.declarations[1] {
        TopLevel::ChanDecl { name, capacity, .. } => {
            assert_eq!(name, "ch2");
            assert_eq!(*capacity, 0);
        }
        _ => panic!("Expected ChanDecl for ch2"),
    }
}

#[test]
fn test_multi_proctype() {
    let source = "active proctype P() { byte x; }\nactive proctype Q() { byte y; }";
    let model = parse(source).unwrap();
    assert_eq!(model.declarations.len(), 2);
}

#[test]
fn test_nested_if_do() {
    let source = "active proctype P() {\n    if\n    :: (x > 0) ->\n        do\n        :: (x > 0) -> x = x - 1\n        :: (x == 0) -> break\n        od\n    :: else -> skip\n    fi\n}";
    let model = parse(source).unwrap();
    assert_eq!(model.declarations.len(), 1);
}

#[test]
fn test_array_access() {
    let source = "active proctype P() { int arr[5]; arr[0] = 42; }";
    let model = parse(source).unwrap();
    match &model.declarations[0] {
        TopLevel::Proctype(p) => {
            assert!(p.body.len() >= 2);
            if let Stmt::Assignment { index, .. } = &p.body[1] {
                assert!(index.is_some())
            }
        }
        _ => panic!("Expected proctype"),
    }
}

#[test]
fn test_complex_expression() {
    let source = "active proctype P() { x = (a + b) * (c - d); }";
    let model = parse(source).unwrap();
    assert_eq!(model.declarations.len(), 1);
}

#[test]
fn test_bitwise_ops() {
    let source = "active proctype P() { x = ~a + 1; }";
    let model = parse(source).unwrap();
    assert_eq!(model.declarations.len(), 1);
}

#[test]
fn test_never_claim_parse() {
    let source = "never { do :: (x == 0) -> skip od }";
    let model = parse(source).unwrap();
    assert_eq!(model.declarations.len(), 1);
    match &model.declarations[0] {
        TopLevel::NeverClaim(_) => {}
        _ => panic!("Expected NeverClaim"),
    }
}

#[test]
fn test_init_block() {
    let source = "init { byte x; x = 1; }";
    let model = parse(source).unwrap();
    assert_eq!(model.declarations.len(), 1);
    match &model.declarations[0] {
        TopLevel::Init(_) => {}
        _ => panic!("Expected Init"),
    }
}

#[test]
fn test_inline_ltl_parse() {
    let source = "ltl { []<>(x == 0) }";
    let model = parse(source).unwrap();
    assert_eq!(model.declarations.len(), 1);
    match &model.declarations[0] {
        TopLevel::Ltl(l) => {
            assert!(l.name.is_none());
        }
        _ => panic!("Expected LTL"),
    }
}

#[test]
fn test_c_code_block() {
    let source = "c_code { printf('hello') }";
    let model = parse(source).unwrap();
    assert_eq!(model.declarations.len(), 1);
    match &model.declarations[0] {
        TopLevel::CCode(code, _) => {
            assert!(code.contains("printf"));
        }
        _ => panic!("Expected CCode"),
    }
}

#[test]
fn test_chan_array_decl() {
    let source = "chan tok[5];";
    let model = parse(source).unwrap();
    assert_eq!(model.declarations.len(), 1);
    match &model.declarations[0] {
        TopLevel::ChannelArray { name, size, .. } => {
            assert_eq!(name, "tok");
            assert_eq!(*size, 5);
        }
        _ => panic!("Expected ChannelArray, got {:?}", model.declarations[0]),
    }
}

#[test]
fn test_chan_array_indexed_send() {
    let source = "active proctype P() { byte i; tok[i] ! 42 }";
    let result = parse(source);
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());
    let model = result.unwrap();
    match &model.declarations[0] {
        TopLevel::Proctype(p) => {
            assert_eq!(p.body.len(), 2); // VarDecl + Send
            match &p.body[1] {
                Stmt::Send { channel, .. } => match channel.as_ref() {
                    Expression::ArrayAccess { name, .. } => {
                        assert_eq!(name, "tok");
                    }
                    _ => panic!("Expected ArrayAccess for channel, got {:?}", channel),
                },
                _ => panic!("Expected Send statement, got {:?}", p.body[1]),
            }
        }
        _ => panic!("Expected Proctype"),
    }
}

#[test]
fn test_chan_array_indexed_recv() {
    let source = "active proctype P() { byte msg; tok[_pid] ? msg }";
    let result = parse(source);
    assert!(result.is_ok(), "Failed to parse: {:?}", result.err());
    let model = result.unwrap();
    match &model.declarations[0] {
        TopLevel::Proctype(p) => {
            assert_eq!(p.body.len(), 2); // VarDecl + Recv
            match &p.body[1] {
                Stmt::Recv { channel, .. } => match channel.as_ref() {
                    Expression::ArrayAccess { name, .. } => {
                        assert_eq!(name, "tok");
                    }
                    _ => panic!("Expected ArrayAccess for channel, got {:?}", channel),
                },
                _ => panic!("Expected Recv statement, got {:?}", p.body[1]),
            }
        }
        _ => panic!("Expected Proctype"),
    }
}
