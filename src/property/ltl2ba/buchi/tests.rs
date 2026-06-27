use super::*;

#[test]
fn test_trivial_accepting() {
    let auto = trivial_accepting();
    assert_eq!(auto.num_states, 1);
    assert!(auto.is_accepting(0));
}

#[test]
fn test_always_pattern() {
    let formula = LtlFormula::Always(Box::new(LtlFormula::Atom("p".to_string())));
    let auto = to_buchi(&formula).unwrap();
    assert_eq!(auto.num_states, 2);
    assert!(auto.is_accepting(0));
    assert!(!auto.is_accepting(1));
}

#[test]
fn test_eventually_pattern() {
    let formula = LtlFormula::Eventually(Box::new(LtlFormula::Atom("p".to_string())));
    let auto = to_buchi(&formula).unwrap();
    assert_eq!(auto.num_states, 2);
    assert!(!auto.is_accepting(0));
    assert!(auto.is_accepting(1));
}

#[test]
fn test_next_pattern() {
    let formula = LtlFormula::Next(Box::new(LtlFormula::Atom("p".to_string())));
    let auto = to_buchi(&formula).unwrap();
    assert_eq!(auto.num_states, 3);
    assert!(auto.is_accepting(1));
}

#[test]
fn test_nested_temporal_error() {
    let formula = LtlFormula::Always(Box::new(LtlFormula::Eventually(Box::new(
        LtlFormula::Atom("p".to_string()),
    ))));
    let result = to_buchi(&formula);
    assert!(matches!(result, Err(LtlError::NestedTemporal { .. })));
}

#[test]
fn test_conjunction() {
    let formula = LtlFormula::And(
        Box::new(LtlFormula::Atom("p".to_string())),
        Box::new(LtlFormula::Atom("q".to_string())),
    );
    let auto = to_buchi(&formula).unwrap();
    assert_eq!(auto.num_states, 1);
}

#[test]
fn test_disjunction() {
    let formula = LtlFormula::Or(
        Box::new(LtlFormula::Atom("p".to_string())),
        Box::new(LtlFormula::Atom("q".to_string())),
    );
    let auto = to_buchi(&formula).unwrap();
    assert_eq!(auto.num_states, 1);
}

#[test]
fn test_true_false() {
    assert!(to_buchi(&LtlFormula::True).unwrap().is_accepting(0));
    assert!(!to_buchi(&LtlFormula::False).unwrap().is_accepting(0));
}

#[test]
fn test_complex_negation() {
    let formula = LtlFormula::Not(Box::new(LtlFormula::Always(Box::new(LtlFormula::Atom(
        "p".to_string(),
    )))));
    let result = to_buchi(&formula);
    assert!(matches!(result, Err(LtlError::UnsupportedOperator { .. })));
}

#[test]
fn test_complex_always() {
    let formula = LtlFormula::Always(Box::new(LtlFormula::And(
        Box::new(LtlFormula::Atom("p".to_string())),
        Box::new(LtlFormula::Atom("q".to_string())),
    )));
    let result = to_buchi(&formula);
    assert!(matches!(result, Err(LtlError::UnsupportedOperator { .. })));
}

#[test]
fn test_complex_eventually() {
    let formula = LtlFormula::Eventually(Box::new(LtlFormula::And(
        Box::new(LtlFormula::Atom("p".to_string())),
        Box::new(LtlFormula::Atom("q".to_string())),
    )));
    let result = to_buchi(&formula);
    assert!(matches!(result, Err(LtlError::UnsupportedOperator { .. })));
}

#[test]
fn test_atom_pattern() {
    let auto = atom_to_buchi(&LtlFormula::Atom("p".to_string()));
    assert_eq!(auto.num_states, 1);
    assert!(auto.is_accepting(0));
}

#[test]
fn test_trivial_rejecting() {
    let auto = trivial_rejecting();
    assert_eq!(auto.num_states, 1);
    assert!(!auto.is_accepting(0));
}

#[test]
fn test_product_conjunction_disjunction() {
    let left = always_to_buchi(&LtlFormula::Atom("p".to_string()));
    let right = always_to_buchi(&LtlFormula::Atom("q".to_string()));

    let conj = product_conjunction(&left, &right);
    assert_eq!(conj.num_states, 4);

    let disj = product_disjunction(&left, &right);
    assert_eq!(disj.num_states, 4);
}

#[test]
fn test_complex_conjunction() {
    let formula = LtlFormula::And(
        Box::new(LtlFormula::Always(Box::new(LtlFormula::Atom(
            "p".to_string(),
        )))),
        Box::new(LtlFormula::Atom("q".to_string())),
    );
    let result = to_buchi(&formula);
    assert!(matches!(result, Err(LtlError::UnsupportedOperator { .. })));
}
