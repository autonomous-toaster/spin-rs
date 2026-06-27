//! LTL formula AST.

use std::collections::HashMap;
use std::fmt;

/// LTL formula representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LtlFormula {
    /// True constant
    True,
    /// False constant
    False,
    /// Atomic proposition (e.g., "x == 0", "flag")
    Atom(String),
    /// Negation: !p
    Not(Box<LtlFormula>),
    /// Conjunction: p && q
    And(Box<LtlFormula>, Box<LtlFormula>),
    /// Disjunction: p || q
    Or(Box<LtlFormula>, Box<LtlFormula>),
    /// Always/Globally: []p
    Always(Box<LtlFormula>),
    /// Eventually/Finally: <>p
    Eventually(Box<LtlFormula>),
    /// Next: Xp
    Next(Box<LtlFormula>),
}

impl LtlFormula {
    /// Collect all atomic propositions from the formula.
    pub fn collect_atoms(&self) -> HashMap<String, u32> {
        let mut atoms = HashMap::new();
        self.collect_atoms_recursive(&mut atoms);
        atoms
    }

    fn collect_atoms_recursive(&self, atoms: &mut HashMap<String, u32>) {
        match self {
            LtlFormula::Atom(name) => {
                let next_id = (atoms.len() + 1) as u32;
                atoms.entry(name.clone()).or_insert(next_id);
            }
            LtlFormula::Not(f) => f.collect_atoms_recursive(atoms),
            LtlFormula::And(f1, f2) | LtlFormula::Or(f1, f2) => {
                f1.collect_atoms_recursive(atoms);
                f2.collect_atoms_recursive(atoms);
            }
            LtlFormula::Always(f) | LtlFormula::Eventually(f) | LtlFormula::Next(f) => {
                f.collect_atoms_recursive(atoms);
            }
            LtlFormula::True | LtlFormula::False => {}
        }
    }

    /// Check if the formula contains any temporal operators.
    pub fn is_temporal(&self) -> bool {
        match self {
            LtlFormula::Always(_) | LtlFormula::Eventually(_) | LtlFormula::Next(_) => true,
            LtlFormula::Not(f) => f.is_temporal(),
            LtlFormula::And(f1, f2) | LtlFormula::Or(f1, f2) => {
                f1.is_temporal() || f2.is_temporal()
            }
            LtlFormula::True | LtlFormula::False | LtlFormula::Atom(_) => false,
        }
    }

    /// Check if the formula is atomic (just an Atom, True, or False).
    pub fn is_atomic(&self) -> bool {
        matches!(
            self,
            LtlFormula::Atom(_) | LtlFormula::True | LtlFormula::False
        )
    }
}

impl fmt::Display for LtlFormula {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LtlFormula::True => write!(fmt, "true"),
            LtlFormula::False => write!(fmt, "false"),
            LtlFormula::Atom(s) => write!(fmt, "{}", s),
            LtlFormula::Not(f) => write!(fmt, "!({})", f),
            LtlFormula::And(f1, f2) => write!(fmt, "({} && {})", f1, f2),
            LtlFormula::Or(f1, f2) => write!(fmt, "({} || {})", f1, f2),
            LtlFormula::Always(f) => write!(fmt, "[]{}", f),
            LtlFormula::Eventually(f) => write!(fmt, "<>{}", f),
            LtlFormula::Next(f) => write!(fmt, "X{}", f),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_atoms() {
        let formula = LtlFormula::And(
            Box::new(LtlFormula::Atom("x == 0".to_string())),
            Box::new(LtlFormula::Atom("y > 1".to_string())),
        );
        let atoms = formula.collect_atoms();
        assert_eq!(atoms.len(), 2);
        assert!(atoms.contains_key("x == 0"));
        assert!(atoms.contains_key("y > 1"));
    }

    #[test]
    fn test_is_temporal() {
        assert!(LtlFormula::Always(Box::new(LtlFormula::Atom("p".to_string()))).is_temporal());
        assert!(LtlFormula::Eventually(Box::new(LtlFormula::Atom("p".to_string()))).is_temporal());
        assert!(LtlFormula::Next(Box::new(LtlFormula::Atom("p".to_string()))).is_temporal());
        assert!(!LtlFormula::Atom("p".to_string()).is_temporal());
        assert!(!LtlFormula::True.is_temporal());
        assert!(!LtlFormula::False.is_temporal());
    }

    #[test]
    fn test_is_atomic() {
        assert!(LtlFormula::Atom("p".to_string()).is_atomic());
        assert!(LtlFormula::True.is_atomic());
        assert!(LtlFormula::False.is_atomic());
        assert!(!LtlFormula::Not(Box::new(LtlFormula::Atom("p".to_string()))).is_atomic());
        assert!(!LtlFormula::Always(Box::new(LtlFormula::Atom("p".to_string()))).is_atomic());
    }
}
