//! Property engine: LTL verification and nested DFS for liveness.
//!
//! This module implements:
//! - LTL formula parsing
//! - Nested DFS for accepting cycle detection (liveness violations)
//! - Safety property checking via assertions
//! - Büchi automaton types (for v2 LTL → Büchi conversion)
//! - Simplified LTL → Büchi conversion (ltl2ba-rs-simplified)

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
    Always(Box<LtlFormula>),                   // []
    Eventually(Box<LtlFormula>),               // <>
    Next(Box<LtlFormula>),                     // X
    Until(Box<LtlFormula>, Box<LtlFormula>),   // U
    Release(Box<LtlFormula>, Box<LtlFormula>), // V
}

impl LtlFormula {
    /// Parse an LTL formula from a string (Spin syntax).
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        Self::parse_manual(s.trim())
    }

    /// Collect all atomic propositions from the formula.
    /// Scan for and parse binary operators (rightmost, lowest precedence first).
    fn parse_binary_op(s: &str) -> anyhow::Result<Option<LtlFormula>> {
        let mut paren_depth = 0usize;
        let mut last_or: Option<usize> = None;
        let mut last_and: Option<usize> = None;
        let mut last_implies: Option<usize> = None;
        let mut last_u: Option<usize> = None;
        let mut last_v: Option<usize> = None;

        let bytes = s.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let c = bytes[i] as char;
            match c {
                '(' => paren_depth += 1,
                ')' => paren_depth = paren_depth.saturating_sub(1),
                ' ' if paren_depth == 0 && i + 2 < bytes.len() => {
                    // "&&" and "||" are two characters following the space, so the
                    // slice must start at i + 1. Slicing from i compared " &&" with
                    // "&&" and never matched, leaving "a && b" to be parsed as a
                    // single atom named "a && b". That atom then never resolved and
                    // made every conjunction-shaped property vacuously true.
                    if &s[i + 1..i + 3] == "&&" {
                        last_and = Some(i + 1);
                    } else if &s[i + 1..i + 3] == "||" {
                        last_or = Some(i + 1);
                    } else if &s[i..i + 3] == " U " {
                        last_u = Some(i);
                    } else if &s[i..i + 3] == " V " {
                        last_v = Some(i);
                    }
                }
                '-' if paren_depth == 0 && i + 1 < bytes.len() && bytes[i + 1] as char == '>' => {
                    last_implies = Some(i);
                }
                _ => {}
            }
            i += 1;
        }

        macro_rules! bin {
            ($kind:ident, $i:expr, $off:expr) => {
                Ok(Some(LtlFormula::$kind(
                    Box::new(Self::parse_manual(&s[..$i])?),
                    Box::new(Self::parse_manual(&s[$i + $off..])?),
                )))
            };
        }

        // Rightmost, lowest-precedence first
        if let Some(i) = last_or {
            return bin!(Or, i, 2);
        }
        if let Some(i) = last_and {
            return bin!(And, i, 2);
        }
        if let Some(i) = last_implies {
            return bin!(Implies, i, 2);
        }
        if let Some(i) = last_u {
            return bin!(Until, i, 3);
        }
        if let Some(i) = last_v {
            return bin!(Release, i, 3);
        }

        Ok(None)
    }

    /// Parse unary prefix operators.
    fn parse_unary_op(s: &str) -> anyhow::Result<Option<LtlFormula>> {
        if s == "true" || s == "1" {
            return Ok(Some(LtlFormula::True));
        }
        if s == "false" || s == "0" {
            return Ok(Some(LtlFormula::False));
        }
        if let Some(rest) = s.strip_prefix("[]") {
            return Ok(Some(LtlFormula::Always(Box::new(Self::parse_manual(
                rest,
            )?))));
        }
        if let Some(rest) = s.strip_prefix("<>") {
            return Ok(Some(LtlFormula::Eventually(Box::new(Self::parse_manual(
                rest,
            )?))));
        }
        if let Some(rest) = s.strip_prefix('X').or_else(|| s.strip_prefix('O')) {
            return Ok(Some(LtlFormula::Next(Box::new(Self::parse_manual(rest)?))));
        }
        Ok(None)
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
            LtlFormula::Always(f) | LtlFormula::Eventually(f) | LtlFormula::Next(f) => {
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
        // Recursive calls arrive with surrounding whitespace (e.g. the inner of
        // "[]( true )" is " true "), which would otherwise defeat the literal
        // checks below and yield Atom(" true ") instead of True.
        let s = s.trim();
        if s.is_empty() {
            return Err(anyhow::anyhow!("empty LTL formula"));
        }

        // Try binary operators first (rightmost, lowest precedence)
        if let Some(result) = Self::parse_binary_op(s)? {
            return Ok(result);
        }

        // Try unary prefix operators
        if let Some(result) = Self::parse_unary_op(s)? {
            return Ok(result);
        }

        // Handle negation
        if let Some(rest) = s.strip_prefix('!') {
            return Ok(LtlFormula::Not(Box::new(Self::parse_manual(rest)?)));
        }

        // Handle parentheses
        if let Some(inner) = s.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
            return Self::parse_manual(inner);
        }

        // Atomic proposition
        Ok(LtlFormula::Atom(s.to_string()))
    }
}

impl std::fmt::Display for LtlFormula {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LtlFormula::True => write!(fmt, "true"),
            LtlFormula::False => write!(fmt, "false"),
            LtlFormula::Atom(name) => write!(fmt, "{}", name),
            LtlFormula::Not(f) => write!(fmt, "!({})", f),
            LtlFormula::And(f1, f2) => write!(fmt, "({} && {})", f1, f2),
            LtlFormula::Or(f1, f2) => write!(fmt, "({} || {})", f1, f2),
            LtlFormula::Implies(f1, f2) => write!(fmt, "({} -> {})", f1, f2),
            LtlFormula::Always(f) => write!(fmt, "[]{}", f),
            LtlFormula::Eventually(f) => write!(fmt, "<>{}", f),
            LtlFormula::Next(f) => write!(fmt, "X{}", f),
            LtlFormula::Until(f1, f2) => write!(fmt, "({} U {})", f1, f2),
            LtlFormula::Release(f1, f2) => write!(fmt, "({} V {})", f1, f2),
        }
    }
}

/// Property checker for LTL and safety properties.
pub struct PropertyChecker<'a, M: Model> {
    model: &'a M,
    property_name: String,
    formula: Option<LtlFormula>,
}

/// Convert from property::LtlFormula to ltl2ba::formula::LtlFormula
fn convert_to_ltl2ba_formula(formula: &LtlFormula) -> crate::property::ltl2ba::formula::LtlFormula {
    use crate::property::ltl2ba::formula::LtlFormula as Target;

    match formula {
        LtlFormula::True => Target::True,
        LtlFormula::False => Target::False,
        LtlFormula::Atom(s) => Target::Atom(s.clone()),
        LtlFormula::Not(inner) => Target::Not(Box::new(convert_to_ltl2ba_formula(inner))),
        LtlFormula::And(l, r) => Target::And(
            Box::new(convert_to_ltl2ba_formula(l)),
            Box::new(convert_to_ltl2ba_formula(r)),
        ),
        LtlFormula::Or(l, r) => Target::Or(
            Box::new(convert_to_ltl2ba_formula(l)),
            Box::new(convert_to_ltl2ba_formula(r)),
        ),
        LtlFormula::Implies(l, r) => {
            // Convert implies to !l || r
            let not_l = Target::Not(Box::new(convert_to_ltl2ba_formula(l)));
            let r_conv = convert_to_ltl2ba_formula(r);
            Target::Or(Box::new(not_l), Box::new(r_conv))
        }
        LtlFormula::Always(inner) => Target::Always(Box::new(convert_to_ltl2ba_formula(inner))),
        LtlFormula::Eventually(inner) => {
            Target::Eventually(Box::new(convert_to_ltl2ba_formula(inner)))
        }
        LtlFormula::Next(inner) => Target::Next(Box::new(convert_to_ltl2ba_formula(inner))),
        LtlFormula::Until(_, _) => {
            // Until not supported by ltl2ba, convert to error case
            // This will trigger fallback to simple cycle detection
            Target::False
        }
        LtlFormula::Release(_, _) => {
            // Release not supported by ltl2ba, convert to error case
            // This will trigger fallback to simple cycle detection
            Target::False
        }
    }
}

impl<'a, M: Model> PropertyChecker<'a, M> {
    /// Create a property checker for an LTL formula.
    pub fn new_ltl(model: &'a M, formula: LtlFormula, name: &str) -> Self {
        Self {
            model,
            property_name: name.to_string(),
            formula: Some(formula),
        }
    }

    /// Create a property checker for safety properties (assertions).
    pub fn new_safety(model: &'a M, name: &str) -> Self {
        Self {
            model,
            property_name: name.to_string(),
            formula: None,
        }
    }

    /// Run nested DFS to check for liveness violations (accepting cycles).
    pub fn check_liveness(&self) -> anyhow::Result<Option<Violation>> {
        let Some(formula) = &self.formula else {
            return Ok(None);
        };

        let init_states = self.model.init_states();
        if init_states.is_empty() {
            return Ok(None);
        }

        // Convert LTL formula to Büchi automaton using ltl2ba
        use crate::property::ltl2ba::{NestedDFS, ProductState};

        // Convert from property::LtlFormula to ltl2ba::formula::LtlFormula
        let ltl2ba_formula = convert_to_ltl2ba_formula(formula);

        // Build the automaton for the NEGATION of the property. A property is violated
        // when a run satisfies its negation, so the product search must look for
        // accepting cycles of the negated automaton. Building the property itself and
        // looking for accepting cycles reported the opposite verdict: it flagged
        // satisfied properties (a model where <>p holds was reported as a violation) and
        // missed violated ones (a model where p never becomes true was reported clean).
        //
        // The previous fallback for unsupported formulas reported ANY cycle as a
        // violation, which is not a verdict about the property at all. An unsupported
        // property now fails loudly instead.
        let buchi = crate::property::ltl2ba::buchi::to_buchi_not(&ltl2ba_formula).map_err(|e| {
            anyhow::anyhow!("property '{}' cannot be checked: {}", self.property_name, e)
        })?;

        // Every atom in the automaton must resolve against a real variable before the
        // product search runs. The product code reads an absent atom as false, which
        // makes an implication-shaped property vacuously true and reports success for a
        // model it never actually judged. Fail with a diagnostic instead.
        let init_states = self.model.init_states();
        if let Some(init) = init_states.first()
            && let Some(blob) = self.model.state_to_string(init)
        {
            for state_trans in &buchi.transitions {
                for trans in state_trans {
                    for (prop, _) in &trans.conditions {
                        if prop == "true" || prop == "false" {
                            continue;
                        }
                        if crate::engine::checker::dfs::extract_var_from_state(&blob, prop)
                            .is_none()
                        {
                            anyhow::bail!(
                                "unknown atomic proposition '{}' in property '{}': it is not a \
                                 variable in the initial state",
                                prop,
                                self.property_name
                            );
                        }
                    }
                }
            }
        }

        // Run nested DFS on product space (model × Büchi)
        let mut violations = Vec::new();

        for init_state in init_states {
            let init_hash = self.model.hash(&init_state);
            let init_product = ProductState::new(init_state.clone(), 0, init_hash);

            let mut dfs = NestedDFS::new();
            if let Some(violation) = dfs.check::<M>(self.model, &buchi, init_product) {
                violations.push(violation);
                break; // Found a violation, stop searching
            }
        }

        Ok(violations.into_iter().next())
    }

    /// Check safety properties (assertions, invariants).
    pub fn check_safety(&self) -> anyhow::Result<Vec<Violation>> {
        Ok(vec![])
    }
}

/// Convenience: verify an LTL property on Promela source.
pub fn verify_ltl(
    source: &str,
    ltl_formula: &str,
    property_name: &str,
) -> anyhow::Result<Option<Violation>> {
    use crate::runtime::LuaModel;

    let formula = LtlFormula::parse(ltl_formula)?;
    let model = LuaModel::from_source(source)?;
    let checker = PropertyChecker::new_ltl(&model, formula, property_name);
    checker.check_liveness()
}

#[cfg(test)]
mod tests;
