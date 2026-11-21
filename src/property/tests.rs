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
    // <>(false) is unsatisfiable, so it is violated in every run: verifying it must
    // fail. This assertion used to be the opposite because the product search built the
    // automaton for the property instead of its negation. Real spin reports 1 error for
    // this model and property.
    let source = "active proctype P() { byte x; x = 1; }";
    let result = verify_ltl(source, "<>(false)", "never").unwrap();
    assert!(
        result.is_some(),
        "<>(false) can never hold, so it must be refuted"
    );
}

#[test]
fn test_ltl_liveness_verdict_is_not_inverted() {
    // An accepting cycle of the property automaton means the property HOLDS. Before the
    // negation was introduced these two models produced opposite answers: the satisfied
    // property was refuted and the violated one was accepted.
    let violated = "bool p = 0;\nactive proctype P() { do :: (1) -> p = 0; od }";
    let satisfied = "bool p = 0;\nactive proctype P() { do :: (1) -> p = 1; od }";

    let v = verify_ltl(violated, "<>(p)", "l0").unwrap();
    let s = verify_ltl(satisfied, "<>(p)", "l0").unwrap();

    assert!(v.is_some(), "<>(p) never holds here, so it must be refuted");
    assert!(s.is_none(), "<>(p) holds here, so it must not be refuted");
}

#[test]
fn test_top_level_eventually_over_finite_run_is_refutable() {
    // A terminating model has no cycle, so a product search that only reports cycles it
    // finds can never refute []!p. Promela lets the never claim stutter at a terminal
    // state, and without that stutter every violated <> over a finite run came back green.
    let violated = "bool p = 0;\nactive proctype P() { do :: (1) -> skip; break od }";
    let satisfied = "bool p = 0;\nactive proctype P() { do :: (1) -> p = 1; break od }";
    assert!(
        verify_ltl(violated, "<>(p)", "l0").unwrap().is_some(),
        "<>(p) never holds in a terminating run, so it must be refuted"
    );
    assert!(
        verify_ltl(satisfied, "<>(p)", "l0").unwrap().is_none(),
        "<>(p) holds here, so it must not be refuted"
    );
}

#[test]
fn test_satisfied_conditional_is_not_an_error() {
    // [] (a -> <>b) is the shape veriplan emits for every conditional constraint. When a
    // never fires the property HOLDS, so reporting an error here flagged a satisfied
    // property as invalid.
    let source = "bool a = 0;\nbool b = 0;\nactive proctype P() { do :: (1) -> skip; break od }\nltl p0 { [] (a -> <> (b)) }";
    let result = crate::runtime::verify(source).unwrap();
    assert_eq!(result.errors, 0, "a never fires, so the conditional holds");
    assert!(
        result.unverified.is_empty(),
        "the shape must be decided, not skipped: {:?}",
        result.unverified
    );
}
