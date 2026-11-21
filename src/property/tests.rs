use super::*;

#[test]
fn test_ltl_parse_always() {
    let formula = LtlFormula::parse("[]x == 0").unwrap();
    assert!(matches!(formula, LtlFormula::Always(_)));
}

#[test]
fn test_ltl_parse_eventually() {
    let formula = LtlFormula::parse("<>x == 0").unwrap();
    assert!(matches!(formula, LtlFormula::Eventually(_)));
}

#[test]
fn test_ltl_parse_until() {
    let formula = LtlFormula::parse("(x >= 0) U (x == 0)").unwrap();
    assert!(matches!(formula, LtlFormula::Until(_, _)));
}

#[test]
fn test_ltl_parse_implies() {
    let formula = LtlFormula::parse("p -> q").unwrap();
    assert!(matches!(formula, LtlFormula::Implies(_, _)));
}

#[test]
fn test_ltl_parse_complex() {
    let formula = LtlFormula::parse("[](p -> <>q)").unwrap();
    assert!(matches!(formula, LtlFormula::Always(_)));
}

#[test]
fn test_ltl_to_string() {
    let formula = LtlFormula::parse("[]x == 0").unwrap();
    let s = formula.to_string();
    assert!(s.contains("[]"));
}

#[test]
fn test_ltl_display_true() {
    assert_eq!(LtlFormula::True.to_string(), "true");
}

#[test]
fn test_ltl_display_false() {
    assert_eq!(LtlFormula::False.to_string(), "false");
}

#[test]
fn test_ltl_display_atom() {
    assert_eq!(LtlFormula::Atom("x".to_string()).to_string(), "x");
}

#[test]
fn test_ltl_display_not() {
    let f = LtlFormula::Not(Box::new(LtlFormula::Atom("p".to_string())));
    assert_eq!(f.to_string(), "!(p)");
}

#[test]
fn test_ltl_display_and() {
    let f = LtlFormula::And(
        Box::new(LtlFormula::Atom("p".to_string())),
        Box::new(LtlFormula::Atom("q".to_string())),
    );
    assert_eq!(f.to_string(), "(p && q)");
}

#[test]
fn test_ltl_display_or() {
    let f = LtlFormula::Or(
        Box::new(LtlFormula::Atom("p".to_string())),
        Box::new(LtlFormula::Atom("q".to_string())),
    );
    assert_eq!(f.to_string(), "(p || q)");
}

#[test]
fn test_ltl_display_implies() {
    let f = LtlFormula::Implies(
        Box::new(LtlFormula::Atom("p".to_string())),
        Box::new(LtlFormula::Atom("q".to_string())),
    );
    assert_eq!(f.to_string(), "(p -> q)");
}

#[test]
fn test_ltl_display_always() {
    let f = LtlFormula::Always(Box::new(LtlFormula::Atom("p".to_string())));
    assert_eq!(f.to_string(), "[]p");
}

#[test]
fn test_ltl_display_eventually() {
    let f = LtlFormula::Eventually(Box::new(LtlFormula::Atom("p".to_string())));
    assert_eq!(f.to_string(), "<>p");
}

#[test]
fn test_ltl_display_next() {
    let f = LtlFormula::Next(Box::new(LtlFormula::Atom("p".to_string())));
    assert_eq!(f.to_string(), "Xp");
}

#[test]
fn test_ltl_display_until() {
    let f = LtlFormula::Until(
        Box::new(LtlFormula::Atom("p".to_string())),
        Box::new(LtlFormula::Atom("q".to_string())),
    );
    assert_eq!(f.to_string(), "(p U q)");
}

#[test]
fn test_ltl_display_release() {
    let f = LtlFormula::Release(
        Box::new(LtlFormula::Atom("p".to_string())),
        Box::new(LtlFormula::Atom("q".to_string())),
    );
    assert_eq!(f.to_string(), "(p V q)");
}

#[test]
fn test_collect_atoms_simple() {
    let f = LtlFormula::Atom("p".to_string());
    let atoms = f.collect_atoms();
    assert!(atoms.contains_key("p"));
    assert_eq!(atoms.len(), 1);
}

#[test]
fn test_collect_atoms_temporal() {
    let f = LtlFormula::parse("[](p)").unwrap();
    let atoms = f.collect_atoms();
    assert!(atoms.contains_key("p"));
}

#[test]
fn test_verify_ltl_safety() {
    // [](true) holds in every state, so a full verification must report no error.
    // This assertion is a real outcome, not a placeholder. Whitespace inside the
    // parentheses previously produced an atom with surrounding spaces that evaluated
    // as false, reporting a spurious violation of a tautology.
    let source = "active proctype P() { byte x; x = 1; }\nltl p0 { [] ( true ) }";
    let result = crate::runtime::verify(source).unwrap();
    assert_eq!(result.errors, 0, "[] ( true ) must never be violated");
}

#[test]
fn test_verify_ltl_empty_model() {
    let source = "/* no processes */";
    let result = verify_ltl(source, "[](true)", "empty").unwrap();
    assert!(result.is_none());
}

#[test]
fn test_verify_ltl_false_formula() {
    let source = "active proctype P() { byte x; x = 1; }";
    let result = verify_ltl(source, "<>(false)", "never").unwrap();
    assert!(result.is_none());
}
