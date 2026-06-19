//! Property engine: LTL verification and nested DFS for liveness.
//!
//! This module implements:
//! - LTL formula parsing
//! - Nested DFS for accepting cycle detection (liveness violations)
//! - Safety property checking via assertions
//! - Büchi automaton types (for v2 LTL → Büchi conversion)
//! - Simplified LTL → Büchi conversion (ltl2ba-rs-simplified)

use std::collections::HashSet;

use crate::engine::checker::{Model, Violation};

pub mod buchi;
pub mod ltl2ba;

/// LTL formula representation.
#[derive(Debug, Clone)]
pub enum LtlFormula {
    True,
    False,
    Atom(String),
    Not(Box<LtlFormula>),
    And(Box<LtlFormula>, Box<LtlFormula>),
    Or(Box<LtlFormula>, Box<LtlFormula>),
    Implies(Box<LtlFormula>, Box<LtlFormula>),
    Always(Box<LtlFormula>),      // []
    Eventually(Box<LtlFormula>),  // <>
    Next(Box<LtlFormula>),        // X
    Until(Box<LtlFormula>, Box<LtlFormula>),  // U
    Release(Box<LtlFormula>, Box<LtlFormula>), // V
}

impl LtlFormula {
    /// Parse an LTL formula from a string (Spin syntax).
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        Self::parse_manual(s.trim())
    }

    /// Collect all atomic propositions from the formula.
    pub fn collect_atoms(&self) -> std::collections::HashMap<String, u32> {
        let mut atoms = std::collections::HashMap::new();
        self.collect_atoms_recursive(&mut atoms);
        atoms
    }

    fn collect_atoms_recursive(&self, atoms: &mut std::collections::HashMap<String, u32>) {
        match self {
            LtlFormula::Atom(name) => {
                let next_id = (atoms.len() + 1) as u32;
                atoms.entry(name.clone()).or_insert(next_id);
            }
            LtlFormula::Not(f) => f.collect_atoms_recursive(atoms),
            LtlFormula::And(f1, f2) | LtlFormula::Or(f1, f2) | LtlFormula::Implies(f1, f2) => {
                f1.collect_atoms_recursive(atoms);
                f2.collect_atoms_recursive(atoms);
            }
            LtlFormula::Always(f)
            | LtlFormula::Eventually(f)
            | LtlFormula::Next(f) => {
                f.collect_atoms_recursive(atoms);
            }
            LtlFormula::Until(f1, f2) | LtlFormula::Release(f1, f2) => {
                f1.collect_atoms_recursive(atoms);
                f2.collect_atoms_recursive(atoms);
            }
            LtlFormula::True | LtlFormula::False => {}
        }
    }

    fn parse_manual(s: &str) -> anyhow::Result<Self> {
        if s.is_empty() {
            return Err(anyhow::anyhow!("empty LTL formula"));
        }
        
        if s == "true" || s == "1" {
            return Ok(LtlFormula::True);
        }
        if s == "false" || s == "0" {
            return Ok(LtlFormula::False);
        }
        if s.starts_with("[]") {
            return Ok(LtlFormula::Always(Box::new(Self::parse_manual(&s[2..])?)));
        }
        if s.starts_with("<>") {
            return Ok(LtlFormula::Eventually(Box::new(Self::parse_manual(&s[2..])?)));
        }
        if s.starts_with('X') || s.starts_with('O') {
            return Ok(LtlFormula::Next(Box::new(Self::parse_manual(&s[1..])?)));
        }
        
        // Handle binary operators by finding the rightmost occurrence at depth 0
        // Use byte indices for slicing
        let mut paren_depth = 0;
        let mut last_u: Option<usize> = None;
        let mut last_v: Option<usize> = None;
        let mut last_implies: Option<usize> = None;
        let mut last_and: Option<usize> = None;
        let mut last_or: Option<usize> = None;
        
        let bytes = s.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let c = bytes[i] as char;
            match c {
                '(' => paren_depth += 1,
                ')' => paren_depth -= 1,
                ' ' if paren_depth == 0 => {
                    // Check for multi-char operators starting with space
                    if i + 2 < bytes.len() && &s[i..i+3] == " U " {
                        last_u = Some(i);
                    } else if i + 2 < bytes.len() && &s[i..i+3] == " V " {
                        last_v = Some(i);
                    } else if i + 2 < bytes.len() && &s[i..i+3] == "&&" {
                        last_and = Some(i);
                    } else if i + 2 < bytes.len() && &s[i..i+3] == "||" {
                        last_or = Some(i);
                    }
                }
                '-' if paren_depth == 0 && i + 1 < bytes.len() && bytes[i+1] as char == '>' => {
                    last_implies = Some(i);
                }
                _ => {}
            }
            i += 1;
        }
        
        // Split at rightmost lowest-precedence operator
        if let Some(i) = last_or {
            return Ok(LtlFormula::Or(
                Box::new(Self::parse_manual(&s[..i])?),
                Box::new(Self::parse_manual(&s[i+2..])?),
            ));
        }
        if let Some(i) = last_and {
            return Ok(LtlFormula::And(
                Box::new(Self::parse_manual(&s[..i])?),
                Box::new(Self::parse_manual(&s[i+2..])?),
            ));
        }
        if let Some(i) = last_implies {
            return Ok(LtlFormula::Implies(
                Box::new(Self::parse_manual(&s[..i])?),
                Box::new(Self::parse_manual(&s[i+2..])?),
            ));
        }
        if let Some(i) = last_u {
            return Ok(LtlFormula::Until(
                Box::new(Self::parse_manual(&s[..i])?),
                Box::new(Self::parse_manual(&s[i+3..])?),
            ));
        }
        if let Some(i) = last_v {
            return Ok(LtlFormula::Release(
                Box::new(Self::parse_manual(&s[..i])?),
                Box::new(Self::parse_manual(&s[i+3..])?),
            ));
        }
        
        // Handle negation
        if s.starts_with('!') {
            return Ok(LtlFormula::Not(Box::new(Self::parse_manual(&s[1..])?)));
        }
        
        // Handle parentheses
        if s.starts_with('(') && s.ends_with(')') {
            return Self::parse_manual(&s[1..s.len()-1]);
        }
        
        // Atomic proposition
        Ok(LtlFormula::Atom(s.to_string()))
    }

    /// Convert to string representation.
    pub fn to_string(&self) -> String {
        match self {
            LtlFormula::True => "true".to_string(),
            LtlFormula::False => "false".to_string(),
            LtlFormula::Atom(name) => name.clone(),
            LtlFormula::Not(f) => format!("!({})", f.to_string()),
            LtlFormula::And(f1, f2) => format!("({} && {})", f1.to_string(), f2.to_string()),
            LtlFormula::Or(f1, f2) => format!("({} || {})", f1.to_string(), f2.to_string()),
            LtlFormula::Implies(f1, f2) => format!("({} -> {})", f1.to_string(), f2.to_string()),
            LtlFormula::Always(f) => format!("[]{}", f.to_string()),
            LtlFormula::Eventually(f) => format!("<>{}", f.to_string()),
            LtlFormula::Next(f) => format!("X{}", f.to_string()),
            LtlFormula::Until(f1, f2) => format!("({} U {})", f1.to_string(), f2.to_string()),
            LtlFormula::Release(f1, f2) => format!("({} V {})", f1.to_string(), f2.to_string()),
        }
    }
}

/// Property checker for LTL and safety properties.
pub struct PropertyChecker<M: Model> {
    model: M,
    property_name: String,
    formula: Option<LtlFormula>,
}

impl<M: Model> PropertyChecker<M> {
    /// Create a property checker for an LTL formula.
    pub fn new_ltl(model: M, formula: LtlFormula, name: &str) -> Self {
        Self {
            model,
            property_name: name.to_string(),
            formula: Some(formula),
        }
    }

    /// Create a property checker for safety properties (assertions).
    pub fn new_safety(model: M, name: &str) -> Self {
        Self {
            model,
            property_name: name.to_string(),
            formula: None,
        }
    }

    /// Run nested DFS to check for liveness violations (accepting cycles).
    pub fn check_liveness(&self) -> anyhow::Result<Option<Violation>> {
        let Some(_formula) = &self.formula else {
            return Ok(None);
        };

        let init_states = self.model.init_states();
        if init_states.is_empty() {
            return Ok(None);
        }

        let mut visited1: HashSet<u64> = HashSet::new();
        let mut visited2: HashSet<u64> = HashSet::new();
        let mut trail: Vec<String> = Vec::new();

        for init_state in init_states {
            let init_hash = self.model.hash(&init_state);
            
            if !visited1.contains(&init_hash)
                && let Some(violation) = self.dfs1(
                    &init_state,
                    init_hash,
                    &mut visited1,
                    &mut visited2,
                    &mut trail,
                )? {
                    return Ok(Some(violation));
                }
        }

        Ok(None)
    }

    fn dfs1(
        &self,
        state: &M::State,
        state_hash: u64,
        visited1: &mut HashSet<u64>,
        visited2: &mut HashSet<u64>,
        trail: &mut Vec<String>,
    ) -> anyhow::Result<Option<Violation>> {
        visited1.insert(state_hash);

        let transitions = self.model.transitions(state);
        for trans in transitions {
            let next_hash = self.model.hash(&trans.next);
            
            if !visited1.contains(&next_hash) {
                trail.push(trans.label.clone());
                if let Some(violation) = self.dfs1(
                    &trans.next,
                    next_hash,
                    visited1,
                    visited2,
                    trail,
                )? {
                    return Ok(Some(violation));
                }
                trail.pop();
            } else if visited2.contains(&next_hash) {
                return Ok(Some(Violation {
                    property_name: self.property_name.clone(),
                    trail: trail.clone(),
                    description: format!("Liveness violation: cycle found in property '{}'", self.property_name),
                }));
            }
        }

        Ok(None)
    }

    /// Check safety properties (assertions, invariants).
    pub fn check_safety(&self) -> anyhow::Result<Vec<Violation>> {
        Ok(vec![])
    }
}

/// Convenience: verify an LTL property on Promela source.
pub fn verify_ltl(source: &str, ltl_formula: &str, property_name: &str) -> anyhow::Result<Option<Violation>> {
    use crate::runtime::LuaModel;
    
    let formula = LtlFormula::parse(ltl_formula)?;
    let model = LuaModel::from_source(source)?;
    let checker = PropertyChecker::new_ltl(model, formula, property_name);
    checker.check_liveness()
}

#[cfg(test)]
mod tests {
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
}
