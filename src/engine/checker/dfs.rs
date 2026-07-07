use super::*;
use crate::engine::fairness::{FairnessMode, FairnessTracker};
use crate::engine::storage::HashCompactStore;

impl<M: Model> Checker<M> {
    pub fn check_dfs(&self) -> CheckResult {
        let start = std::time::Instant::now();
        let init_states = self.model.init_states();

        if init_states.is_empty() {
            return self.empty_result(0.0);
        }

        // Parse LTL formulas into safety checks ([]expr where expr is propositional)
        let safety_checks = self.parse_safety_formulas();

        let mut storage = self.make_storage();
        let mut stack: Vec<(M::State, usize, usize)> = Vec::new(); // (state, depth, parent_index)
        let mut trail: Vec<(String, usize)> = Vec::new(); // (transition_label, parent_index)
        let mut transitions_count = 0;
        let mut violations = Vec::new();

        // Fairness tracking
        let mut fairness = FairnessTracker::new(self.config.fairness_mode);
        let mut fairness_check_counter: usize = 0;

        for s in init_states {
            let h = self.model.hash(&s);
            if storage.insert(h, &s) {
                let idx = trail.len();
                trail.push((String::new(), 0));
                stack.push((s, 0, idx));
            }
        }

        while let Some((state, depth, state_idx)) = stack.pop() {
            if depth >= self.config.max_depth {
                continue;
            }
            if storage.len() >= self.config.max_states {
                break;
            }

            // Check for violations (safety properties / assertions)
            if self.config.check_assertions
                && let Some(desc) = self.model.check_violation(&state)
            {
                let state_trail = self.build_trail(&trail, state_idx);
                violations.push(Violation {
                    property_name: "assertion".to_string(),
                    trail: state_trail,
                    description: desc,
                });
                if violations.len() >= 100 {
                    break;
                }
                continue;
            }

            // Check safety properties ([]expr) during DFS
            if !safety_checks.is_empty()
                && let Some(state_str) = self.model.state_to_string(&state)
            {
                for check in &safety_checks {
                    if !evaluate_propositional(&check.formula, &state_str) {
                        let state_trail = self.build_trail(&trail, state_idx);
                        violations.push(Violation {
                            property_name: check.prop_name.clone(),
                            trail: state_trail,
                            description: format!(
                                "Safety violation: '{}' is false in state {}",
                                check.prop_name, state_str
                            ),
                        });
                        if violations.len() >= 100 {
                            break;
                        }
                    }
                }
            }

            let trans = self.model.transitions(&state);
            transitions_count += trans.len();

            // Record enabled transitions for fairness tracking
            if self.config.fairness_mode != FairnessMode::None {
                let labels: Vec<String> = trans.iter().map(|t| t.label.clone()).collect();
                fairness.record_enabled(&labels);
            }

            for t in trans {
                // Record fired transition for fairness
                if self.config.fairness_mode != FairnessMode::None {
                    fairness.record_fired(&t.label);
                }

                let h = self.model.hash(&t.next);
                if storage.insert(h, &t.next) {
                    let idx = trail.len();
                    trail.push((t.label, state_idx));
                    stack.push((t.next, depth + 1, idx));
                }
            }

            // Periodic fairness check (every 1000 states)
            if self.config.fairness_mode == FairnessMode::Strong {
                fairness_check_counter += 1;
                if fairness_check_counter >= 1000 {
                    fairness_check_counter = 0;
                    let prioritized = fairness.get_prioritized_transitions();
                    for label in &prioritized {
                        let enabled = fairness.enabled_count(label);
                        let fired = fairness.fired_count(label);
                        if enabled > 10 && fired == 0 {
                            violations.push(Violation {
                                property_name: "strong_fairness".to_string(),
                                trail: vec![],
                                description: format!(
                                    "Strong fairness violation: transition '{}' was enabled {} times but never taken",
                                    label, enabled
                                ),
                            });
                            if violations.len() >= 100 {
                                break;
                            }
                        }
                    }
                }
            }
        }

        let elapsed = start.elapsed().as_secs_f64();

        let mut result = CheckResult {
            states_explored: storage.len(),
            states_stored: storage.len(),
            transitions: transitions_count,
            depth_reached: self
                .config
                .max_depth
                .min(stack.iter().map(|(_, d, _)| *d).max().unwrap_or(0)),
            errors: violations.len(),
            violations,
            elapsed_secs: elapsed,
        };

        // Check remaining LTL properties (non-[]p formulas via nested DFS)
        self.check_ltl_properties(&mut result);

        result
    }

    /// Check LTL properties using PropertyChecker (nested DFS).
    /// Skips []expr formulas that were already checked as safety properties during DFS.
    fn check_ltl_properties(&self, result: &mut CheckResult) {
        let formulas = self.model.ltl_formulas();
        if formulas.is_empty() {
            return;
        }

        for ltl_ast in formulas {
            let formula_str = ltl_ast.formula.trim();
            let prop_name = ltl_ast.name.as_deref().unwrap_or("ltl");

            // Skip formulas that were already checked as safety properties during DFS
            if is_safety_formula(formula_str) {
                continue;
            }

            match crate::property::LtlFormula::parse(formula_str) {
                Ok(formula) => {
                    let checker =
                        crate::property::PropertyChecker::new_ltl(&self.model, formula, prop_name);
                    match checker.check_liveness() {
                        Ok(Some(violation)) => {
                            result.errors += 1;
                            result.violations.push(violation);
                        }
                        Ok(None) => {}
                        Err(e) => {
                            log::warn!("LTL check error for '{}': {}", prop_name, e);
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Failed to parse LTL formula '{}': {}", prop_name, e);
                }
            }
        }
    }

    /// Run BFS state exploration.
    pub fn check_bfs(&self) -> CheckResult {
        let start = std::time::Instant::now();
        let init_states = self.model.init_states();

        if init_states.is_empty() {
            return self.empty_result(0.0);
        }

        // Parse LTL formulas into safety checks ([]expr where expr is propositional)
        let safety_checks = self.parse_safety_formulas();

        let mut storage = self.make_storage();
        let mut queue: VecDeque<(M::State, usize, usize)> = VecDeque::new(); // (state, depth, parent_index)
        let mut trail: Vec<(String, usize)> = Vec::new();
        let mut transitions_count = 0;
        let mut violations = Vec::new();
        let mut max_depth = 0;

        // Fairness tracking
        let mut fairness = FairnessTracker::new(self.config.fairness_mode);
        let mut fairness_check_counter: usize = 0;

        for s in init_states {
            let h = self.model.hash(&s);
            if storage.insert(h, &s) {
                let idx = trail.len();
                trail.push((String::new(), 0));
                queue.push_back((s, 0, idx));
            }
        }

        while let Some((state, depth, state_idx)) = queue.pop_front() {
            max_depth = max_depth.max(depth);

            if depth >= self.config.max_depth {
                continue;
            }
            if storage.len() >= self.config.max_states {
                break;
            }

            if self.config.check_assertions
                && let Some(desc) = self.model.check_violation(&state)
            {
                let state_trail = self.build_trail(&trail, state_idx);
                violations.push(Violation {
                    property_name: "assertion".to_string(),
                    trail: state_trail,
                    description: desc,
                });
                if violations.len() >= 100 {
                    break;
                }
                continue;
            }

            // Check safety properties ([]expr) during BFS
            if !safety_checks.is_empty()
                && let Some(state_str) = self.model.state_to_string(&state)
            {
                for check in &safety_checks {
                    if !evaluate_propositional(&check.formula, &state_str) {
                        let state_trail = self.build_trail(&trail, state_idx);
                        violations.push(Violation {
                            property_name: check.prop_name.clone(),
                            trail: state_trail,
                            description: format!(
                                "Safety violation: '{}' is false in state {}",
                                check.prop_name, state_str
                            ),
                        });
                        if violations.len() >= 100 {
                            break;
                        }
                    }
                }
            }

            let trans = self.model.transitions(&state);
            transitions_count += trans.len();

            // Record enabled transitions for fairness tracking
            if self.config.fairness_mode != FairnessMode::None {
                let labels: Vec<String> = trans.iter().map(|t| t.label.clone()).collect();
                fairness.record_enabled(&labels);
            }

            for t in trans {
                // Record fired transition for fairness
                if self.config.fairness_mode != FairnessMode::None {
                    fairness.record_fired(&t.label);
                }

                let h = self.model.hash(&t.next);
                if storage.insert(h, &t.next) {
                    let idx = trail.len();
                    trail.push((t.label, state_idx));
                    queue.push_back((t.next, depth + 1, idx));
                }
            }

            // Periodic fairness check (every 1000 states)
            if self.config.fairness_mode == FairnessMode::Strong {
                fairness_check_counter += 1;
                if fairness_check_counter >= 1000 {
                    fairness_check_counter = 0;
                    let prioritized = fairness.get_prioritized_transitions();
                    for label in &prioritized {
                        let enabled = fairness.enabled_count(label);
                        let fired = fairness.fired_count(label);
                        if enabled > 10 && fired == 0 {
                            violations.push(Violation {
                                property_name: "strong_fairness".to_string(),
                                trail: vec![],
                                description: format!(
                                    "Strong fairness violation: transition '{}' was enabled {} times but never taken",
                                    label, enabled
                                ),
                            });
                            if violations.len() >= 100 {
                                break;
                            }
                        }
                    }
                }
            }
        }

        let elapsed = start.elapsed().as_secs_f64();

        CheckResult {
            states_explored: storage.len(),
            states_stored: storage.len(),
            transitions: transitions_count,
            depth_reached: max_depth,
            errors: violations.len(),
            violations,
            elapsed_secs: elapsed,
        }
    }

    /// Build an error trail from parent pointers.
    fn build_trail(&self, trail: &[(String, usize)], end_idx: usize) -> Vec<String> {
        let mut result = Vec::new();
        let mut idx = end_idx;
        while idx > 0 {
            let (ref label, _) = trail[idx];
            if !label.is_empty() {
                result.push(label.clone());
            }
            idx = trail[idx].1;
            // Guard against runaway loops (malformed trail index)
            if result.len() > 100_000 {
                break;
            }
        }
        // We built from parent back to root, so reverse
        result.reverse();
        result
    }

    fn make_storage(&self) -> Box<dyn StateStore<M::State>> {
        match self.config.storage_mode {
            StorageMode::Exact => Box::new(ExactStore::<M::State>::new()),
            StorageMode::Bitstate => {
                Box::new(BitstateStore::new((self.config.max_states / 8).max(1024)))
            }
            StorageMode::Collapse => Box::new(CollapseStore::<M::State>::new(4)),
            StorageMode::HashCompact => {
                Box::new(HashCompactStore::<M::State>::new(1024))
            }
        }
    }

    fn empty_result(&self, elapsed_secs: f64) -> CheckResult {
        CheckResult {
            states_explored: 0,
            states_stored: 0,
            transitions: 0,
            depth_reached: 0,
            errors: 0,
            violations: vec![],
            elapsed_secs,
        }
    }

    pub fn model(&self) -> &M {
        &self.model
    }

    pub fn check_dfs_old(&self) -> CheckResult {
        self.check_dfs()
    }

    /// Parse LTL formulas into safety checks ([]expr where expr is propositional).
    /// These are checked during DFS by evaluating expr against each state.
    fn parse_safety_formulas(&self) -> Vec<SafetyCheck> {
        let formulas = self.model.ltl_formulas();
        let mut checks = Vec::new();

        for ltl_ast in formulas {
            let formula_str = ltl_ast.formula.trim();
            let prop_name = ltl_ast.name.as_deref().unwrap_or("ltl").to_string();

            // Parse the formula and check if it's []expr where expr is propositional
            if let Ok(formula) = crate::property::LtlFormula::parse(formula_str) {
                if let crate::property::LtlFormula::Always(inner) = &formula {
                    if is_propositional(inner) {
                        checks.push(SafetyCheck {
                            prop_name,
                            formula: *inner.clone(),
                        });
                    }
                }
            }
        }

        checks
    }
}

/// A safety property to check during DFS: []expr where expr is propositional.
struct SafetyCheck {
    /// Property name for error reporting.
    prop_name: String,
    /// The propositional formula (inner of []).
    formula: crate::property::LtlFormula,
}

/// Check if an LTL formula is purely propositional (no temporal operators).
fn is_propositional(formula: &crate::property::LtlFormula) -> bool {
    match formula {
        crate::property::LtlFormula::True | crate::property::LtlFormula::False => true,
        crate::property::LtlFormula::Atom(_) => true,
        crate::property::LtlFormula::Not(inner) => is_propositional(inner),
        crate::property::LtlFormula::And(l, r)
        | crate::property::LtlFormula::Or(l, r)
        | crate::property::LtlFormula::Implies(l, r) => {
            is_propositional(l) && is_propositional(r)
        }
        // Temporal operators
        crate::property::LtlFormula::Always(_)
        | crate::property::LtlFormula::Eventually(_)
        | crate::property::LtlFormula::Next(_)
        | crate::property::LtlFormula::Until(_, _)
        | crate::property::LtlFormula::Release(_, _) => false,
    }
}

/// Evaluate a propositional LTL formula against a state string.
/// Returns true if the formula holds in the state.
fn evaluate_propositional(formula: &crate::property::LtlFormula, state_str: &str) -> bool {
    match formula {
        crate::property::LtlFormula::True => true,
        crate::property::LtlFormula::False => false,
        crate::property::LtlFormula::Atom(name) => {
            let val = extract_var_from_state(state_str, name);
            val != 0
        }
        crate::property::LtlFormula::Not(inner) => !evaluate_propositional(inner, state_str),
        crate::property::LtlFormula::And(l, r) => {
            evaluate_propositional(l, state_str) && evaluate_propositional(r, state_str)
        }
        crate::property::LtlFormula::Or(l, r) => {
            evaluate_propositional(l, state_str) || evaluate_propositional(r, state_str)
        }
        crate::property::LtlFormula::Implies(l, r) => {
            !evaluate_propositional(l, state_str) || evaluate_propositional(r, state_str)
        }
        // Temporal operators inside []expr shouldn't reach here (filtered by is_propositional)
        _ => true,
    }
}

/// Check if a formula string is a safety formula ([]expr where expr is propositional).
fn is_safety_formula(formula_str: &str) -> bool {
    if let Ok(formula) = crate::property::LtlFormula::parse(formula_str) {
        if let crate::property::LtlFormula::Always(inner) = &formula {
            return is_propositional(inner);
        }
    }
    false
}

/// Extract a variable's integer value from a state blob string.
/// State blob format: {key:val,key:val,...} or {"key":val,"key":val,...}
fn extract_var_from_state(state_str: &str, var_name: &str) -> i64 {
    let inner = state_str
        .trim_start_matches('{')
        .trim_end_matches('}')
        .trim();

    // Search for the variable name followed by ':'
    let search_key = format!("\"{}\":", var_name);
    let search_key2 = format!("{}:", var_name);

    let pos = inner.find(&search_key).or_else(|| inner.find(&search_key2));

    if let Some(p) = pos {
        let after_key = &inner[p + search_key.len().min(inner.len() - p)..];
        // Skip past the key (handle both quoted and unquoted)
        let after_colon = if let Some(colon_pos) = after_key.find(':') {
            &after_key[colon_pos + 1..]
        } else {
            after_key
        };
        let after_colon = after_colon.trim();
        // Parse the value (stop at comma, brace, or whitespace)
        let num_str: String = after_colon
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '-')
            .collect();
        num_str.parse::<i64>().unwrap_or(0)
    } else {
        0
    }
}
