use super::*;

impl<M: Model> Checker<M> {
    pub fn check_dfs(&self) -> CheckResult {
        let start = std::time::Instant::now();
        let init_states = self.model.init_states();

        if init_states.is_empty() {
            return self.empty_result(0.0);
        }

        // Parse LTL formulas and identify []p invariants for fast-path checking
        let invariants = self.parse_invariants();

        let mut storage = self.make_storage();
        let mut stack: Vec<(M::State, usize, usize)> = Vec::new(); // (state, depth, parent_index)
        let mut trail: Vec<(String, usize)> = Vec::new(); // (transition_label, parent_index)
        let mut transitions_count = 0;
        let mut violations = Vec::new();

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

            // Check []p invariants during DFS (fast path)
            if !invariants.is_empty()
                && let Some(state_str) = self.model.state_to_string(&state)
            {
                for inv in &invariants {
                    let var_val = extract_var_from_state(&state_str, &inv.var_name);
                    let holds = if inv.expect_nonzero {
                        var_val != 0
                    } else {
                        var_val == 0
                    };
                    if !holds {
                        let state_trail = self.build_trail(&trail, state_idx);
                        violations.push(Violation {
                            property_name: inv.prop_name.clone(),
                            trail: state_trail,
                            description: format!(
                                "[]p violation: '{}' is false in state ({} = {})",
                                inv.prop_name, inv.var_name, var_val
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

            for t in trans {
                let h = self.model.hash(&t.next);
                if storage.insert(h, &t.next) {
                    let idx = trail.len();
                    trail.push((t.label, state_idx));
                    stack.push((t.next, depth + 1, idx));
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
    /// Skips []p formulas that were already checked as invariants during DFS.
    fn check_ltl_properties(&self, result: &mut CheckResult) {
        let formulas = self.model.ltl_formulas();
        if formulas.is_empty() {
            return;
        }

        for ltl_ast in formulas {
            let formula_str = ltl_ast.formula.trim();
            let prop_name = ltl_ast.name.as_deref().unwrap_or("ltl");

            // Skip []p formulas — they were checked as invariants during DFS
            if is_always_atom_formula(formula_str) {
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

        let mut storage = self.make_storage();
        let mut queue: VecDeque<(M::State, usize, usize)> = VecDeque::new(); // (state, depth, parent_index)
        let mut trail: Vec<(String, usize)> = Vec::new();
        let mut transitions_count = 0;
        let mut violations = Vec::new();
        let mut max_depth = 0;

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

            let trans = self.model.transitions(&state);
            transitions_count += trans.len();

            for t in trans {
                let h = self.model.hash(&t.next);
                if storage.insert(h, &t.next) {
                    let idx = trail.len();
                    trail.push((t.label, state_idx));
                    queue.push_back((t.next, depth + 1, idx));
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

    /// Parse LTL formulas and extract []p invariants for fast-path checking.
    fn parse_invariants(&self) -> Vec<InvariantCheck> {
        let formulas = self.model.ltl_formulas();
        let mut invariants = Vec::new();

        for ltl_ast in formulas {
            let formula_str = ltl_ast.formula.trim();
            let prop_name = ltl_ast.name.as_deref().unwrap_or("ltl").to_string();

            // Try to parse as []p or []!p
            if let Some(var_name) = parse_always_atom(formula_str) {
                let (var_name, expect_nonzero) = if let Some(negated) = var_name.strip_prefix('!') {
                    (negated.to_string(), false)
                } else {
                    (var_name, true)
                };
                invariants.push(InvariantCheck {
                    prop_name,
                    var_name,
                    expect_nonzero,
                });
            }
        }

        invariants
    }
}

/// A []p invariant to check during DFS.
struct InvariantCheck {
    /// Property name for error reporting.
    prop_name: String,
    /// Variable name to check.
    var_name: String,
    /// If true, expect variable to be non-zero; if false, expect zero.
    expect_nonzero: bool,
}

/// Check if a formula string is `[]p` or `[]!p` where p is a simple atom.
/// Returns the inner atom (possibly negated) if so, None otherwise.
fn parse_always_atom(formula: &str) -> Option<String> {
    let formula = formula.trim();
    // Match []p or []!p
    if let Some(rest) = formula.strip_prefix("[]") {
        let rest = rest.trim();
        if !rest.is_empty()
            && !rest.contains(' ')
            && !rest.contains('(')
            && !rest.contains(')')
            && !rest.contains('&')
            && !rest.contains('|')
            && !rest.contains('>')
            && !rest.contains('U')
            && !rest.contains('V')
            && !rest.contains('<')
            && !rest.contains('X')
        {
            return Some(rest.to_string());
        }
    }
    None
}

/// Check if a formula string is `[]p` (always p) — used to skip in nested DFS.
fn is_always_atom_formula(formula: &str) -> bool {
    parse_always_atom(formula).is_some()
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
