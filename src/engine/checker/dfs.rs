use super::props::{evaluate_propositional, first_unresolved_atom, is_propositional};
pub(crate) use super::props::{extract_var_from_state, is_safety_formula};
use super::*;
use crate::engine::fairness::{FairnessMode, FairnessTracker};
use crate::engine::storage::HashCompactStore;

impl<M: Model> Checker<M> {
    pub fn check_dfs(&self) -> CheckResult {
        self.check_dfs_inner(None, self.config.max_states, true)
    }

    /// One depth-first traversal.
    ///
    /// When scope is Some, the traversal is confined to that claim: only its safety check is
    /// evaluated, reduction is applied against its visible fields, and budget shortfalls are
    /// attributed to it by name. When scope is None the traversal behaves exactly as it did
    /// before reduction existed - every claim, no reduction, the full budget.
    pub(super) fn check_dfs_inner(
        &self,
        scope: Option<&ReductionScope>,
        budget: usize,
        model_checks: bool,
    ) -> CheckResult {
        let start = std::time::Instant::now();
        let init_states = self.model.init_states();

        if init_states.is_empty() {
            return self.empty_result(0.0);
        }

        // Parse LTL formulas into safety checks ([]expr where expr is propositional).
        // A scoped traversal evaluates only the claim it was created for.
        let safety_checks: Vec<SafetyCheck> = {
            let all = self.parse_safety_formulas();
            match scope {
                Some(s) => all.into_iter().filter(|c| c.prop_name == s.name).collect(),
                None => all,
            }
        };

        let mut storage = self.make_storage();
        let mut stack: Vec<(M::State, usize, usize)> = Vec::new(); // (state, depth, parent_index)
        let mut trail: Vec<(String, usize)> = Vec::new(); // (transition_label, parent_index)
        // Hash of the state each trail entry was created from, so the current search path can be
        // reconstructed by walking parents. Needed by the cycle proviso below.
        let mut path_hashes: Vec<u64> = Vec::new();
        let mut transitions_count = 0;
        let mut violations = Vec::new();
        let mut hit_state_limit = false;
        let mut hit_depth_limit = false;
        let mut hit_early_stop = false;
        // Claims whose formula the checker could not evaluate. Kept per claim, reported once.
        let mut unresolved: std::collections::BTreeMap<String, (UnverifiedKind, String)> =
            std::collections::BTreeMap::new();

        // Fairness tracking
        let mut fairness = FairnessTracker::new(self.config.fairness_mode);
        let mut fairness_check_counter: usize = 0;

        for s in init_states {
            let h = self.model.hash(&s);
            if storage.insert(h, &s) {
                let idx = trail.len();
                trail.push((String::new(), idx));
                path_hashes.push(h);
                stack.push((s, 0, idx));
            }
        }

        while let Some((state, depth, state_idx)) = stack.pop() {
            // Opt-in early stop: one refutation is enough to call the model violated, and the
            // remaining reachable space would be discarded anyway.
            if self.config.stop_at_first_violation && !violations.is_empty() {
                hit_early_stop = true;
                break;
            }
            if depth >= self.config.max_depth {
                // Pruning here leaves successors unexplored, so the search did not close.
                hit_depth_limit = true;
                continue;
            }
            // Check for violations (safety properties / assertions). Model-level checks run on
            // one traversal only: repeating them per claim would report the same deadlock N times.
            if model_checks
                && self.config.check_assertions
                && let Some(desc) = self.model.check_violation(&state)
            {
                let state_trail = self.build_trail(&trail, state_idx);
                let name = model_violation_name(&desc);
                violations.push(Violation {
                    property_name: name.to_string(),
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
                    match evaluate_propositional(&check.formula, &state_str, &self.model, &state) {
                        Some(true) => {}
                        Some(false) => {
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
                        None => {
                            // An atom that does not resolve is not a refutation. Counting it as
                            // one turned a SATISFIED property into a violation: a claim of the
                            // form  [] ( a <= 3 )  came back INVALID because the LTL layer keeps
                            // a comparison as a single opaque atom while the evaluator only
                            // resolves bare names - and pan accepts that model. The claim is
                            // reported as undecided instead.
                            //
                            // Two different things are reported as undecided here and they are
                            // not the same debt. An atom that parses but names nothing the state
                            // can answer is an unsupported property; an atom that does not even
                            // parse is a formula the parser mangled, and saying "no variable
                            // named 'a <'" blames the model for the parser's fault.
                            let (kind, reason) = match first_unresolved_atom(
                                &check.formula,
                                &state_str,
                                &self.model,
                                &state,
                            ) {
                                Some(atom) if crate::parser::parse_expression(&atom).is_err() => (
                                    UnverifiedKind::UnparsableFormula,
                                    format!(
                                        "'{}' could not be parsed: '{}' is not an expression, so the formula was read as a single atom named after a fragment (was: '{}')",
                                        check.prop_name, atom, check.formula
                                    ),
                                ),
                                Some(atom) => (
                                    UnverifiedKind::UnsupportedProperty,
                                    format!(
                                        "cannot decide '{}': the atom '{}' names no state variable (was: '{}')",
                                        check.prop_name, atom, check.formula
                                    ),
                                ),
                                None => (
                                    UnverifiedKind::UnsupportedProperty,
                                    format!(
                                        "cannot decide '{}': the formula contains an atom that does not resolve to a state variable (was: '{}')",
                                        check.prop_name, check.formula
                                    ),
                                ),
                            };
                            unresolved
                                .entry(check.prop_name.clone())
                                .or_insert_with(|| (kind, reason));
                        }
                    }
                }
            }

            // The budget bounds how much of the state space is *stored*, so it is enforced after
            // this state has been examined and before its successors are enumerated. It used to be
            // checked on entry, which meant a budget of one stored the initial state and then
            // examined nothing: every claim with a share of one state was reported starved even
            // when the initial state already refuted it.
            if storage.len() >= budget {
                hit_state_limit = true;
                break;
            }

            let enabled = self.model.transitions(&state);
            transitions_count += enabled.len();

            // Record enabled transitions for fairness tracking
            if self.config.fairness_mode != FairnessMode::None {
                let labels: Vec<String> = enabled.iter().map(|t| t.label.clone()).collect();
                fairness.record_enabled(&labels);
            }

            // Reduction: explore only the ample set, unless this state lies on a cycle.
            //
            // The cycle proviso is not optional. An ample set can be persistent at every state
            // and still lose a transition forever: a process looping invisibly makes each state
            // pick its own transition, the DFS never returns to the state where the other
            // process was dropped, and a violation behind it is never reached. Measured on
            //   active proctype A() { do :: (1) -> skip od }
            //   active proctype B() { do :: (1) -> bad = 1; break od }
            // where the reduced search reported zero errors and the unreduced one found one.
            //
            // A state whose successor is already on the search path closes a cycle, so it
            // explores everything; every reduced graph is then acyclic, which is what makes the
            // persistence argument terminate instead of going round.
            let keep: Vec<usize> = match scope {
                Some(sc) if !sc.liveness => {
                    let mut ancestors = std::collections::HashSet::new();
                    let mut i = state_idx;
                    loop {
                        ancestors.insert(path_hashes[i]);
                        let parent = trail[i].1;
                        if parent == i {
                            break;
                        }
                        i = parent;
                    }
                    let on_cycle = enabled
                        .iter()
                        .any(|t| ancestors.contains(&self.model.hash(&t.next)));
                    if on_cycle {
                        (0..enabled.len()).collect()
                    } else {
                        let deps: Vec<TransitionDeps> =
                            enabled.iter().map(|t| t.deps.clone()).collect();
                        ample_set(&deps, &sc.visible)
                    }
                }
                _ => (0..enabled.len()).collect(),
            };

            for &i in &keep {
                let t = &enabled[i];
                // Record fired transition for fairness
                if self.config.fairness_mode != FairnessMode::None {
                    fairness.record_fired(&t.label);
                }

                let h = self.model.hash(&t.next);
                if storage.insert(h, &t.next) {
                    let idx = trail.len();
                    trail.push((t.label.clone(), state_idx));
                    path_hashes.push(h);
                    stack.push((t.next.clone(), depth + 1, idx));
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

        // A model that declares processes but yields no transitions was never explored.
        // Reporting success here is the null check that makes a whole verification
        // pipeline meaningless, so it is an error, not a pass.
        if model_checks
            && transitions_count == 0
            && self.model.process_count() > 0
            // A traversal stopped by its budget has not established that nothing is enabled; it
            // established that it ran out. Reporting the model as unexplorable because the budget
            // was one state made a small budget look like a broken model.
            && !hit_state_limit
            && !hit_depth_limit
            && !violations.iter().any(|v| v.property_name == "model")
            // A deadlock at the initial state already explains why nothing was explored.
            // Reporting it twice inflated the error count: one blocked model produced two
            // errors where pan reports one, and the second said nothing had been verified
            // when the search had in fact decided the model.
            && !violations.iter().any(|v| v.property_name == "deadlock")
        {
            violations.push(Violation {
                property_name: "model".to_string(),
                trail: vec![],
                description: format!(
                    "no transitions explored: the model declares {} process(es) but none are enabled in the initial state, so nothing was verified",
                    self.model.process_count()
                ),
            });
        }

        // Collapse to one violation per property before reporting.
        let violations = dedup_violations(violations);

        let unresolved: Vec<Unverified> = unresolved
            .into_iter()
            .map(|(property_name, (kind, reason))| Unverified::new(property_name, kind, reason))
            .collect();

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
            unverified: unresolved,
            elapsed_secs: elapsed,
        };

        // A search that stopped at the state limit did not finish. Without this the caller
        // cannot tell "closed, no violations" from "gave up at the cap".
        if hit_depth_limit {
            result.unverified.push(Unverified {
                // A starved traversal belongs to the claim it was checking. Naming it "model"
                // left the claim itself unmentioned, so a caller reading property_name saw a
                // model-level note and could report the claim as satisfied.
                property_name: match scope {
                    Some(s) => s.name.clone(),
                    None => "model".to_string(),
                },
                kind: UnverifiedKind::DepthBudget,
                reason: format!(
                    "search pruned states at the depth limit of {} before the state space closed",
                    self.config.max_depth
                ),
                states_used: storage.len(),
                budget: None,
            });
        }
        if hit_state_limit {
            result.unverified.push(Unverified {
                property_name: match scope {
                    Some(s) => s.name.clone(),
                    None => "model".to_string(),
                },
                kind: UnverifiedKind::StateBudget,
                reason: match scope {
                    Some(_) => format!(
                        "search stopped at the state limit of {} before the state space closed; that is this claim's share of the run's budget",
                        budget
                    ),
                    None => format!(
                        "search stopped at the state limit of {} before the state space closed",
                        budget
                    ),
                },
                // How much this traversal actually stored, and what it was allowed: a caller
                // sizing a budget for a plan needs the distance, not just "used up".
                states_used: storage.len(),
                budget: Some(budget),
            });
        }

        // Check remaining LTL properties (non-[]p formulas via nested DFS). Skipped when an
        // early stop already produced a refutation: the caller asked for the first one, and
        // exploring the rest would defeat that.
        if scope.is_none() {
            if self.config.stop_at_first_violation && result.errors > 0 {
                // The liveness pass is skipped to keep a stopped run cheap, but a claim that was
                // never examined is not a claim that holds. Leaving it absent from the result made
                // a refuted liveness claim read as satisfied to any caller counting entries.
                self.report_skipped_liveness(&mut result);
            } else {
                self.check_ltl_properties(&mut result);
            }
        }

        // With an early stop, every claim that did not produce a violation was left
        // unexplored. Report them individually so a caller cannot read "no violation
        // reported" as "satisfied" - the failure mode this whole contract exists to stop.
        if hit_early_stop && scope.is_none() {
            let decided: std::collections::HashSet<&str> = result
                .violations
                .iter()
                .map(|v| v.property_name.as_str())
                .collect();
            let undecided: Vec<Unverified> = self
                .model
                .ltl_formulas()
                .iter()
                .map(|f| f.name.as_deref().unwrap_or("ltl"))
                .filter(|n| !decided.contains(*n))
                .map(|n| {
                    Unverified::new(
                        n.to_string(),
                        UnverifiedKind::EarlyStop,
                        "search stopped at the first violation, so this claim was not fully explored",
                    )
                })
                .collect();
            result.unverified.extend(undecided);
        }

        result
    }

    /// Check LTL properties using PropertyChecker (nested DFS).
    /// Skips []expr formulas that were already checked as safety properties during DFS.
    /// Name every claim the skipped liveness pass never examined.
    ///
    /// A claim that is absent from both `violations` and `unverified` reads as satisfied, which is
    /// the one thing this type exists to prevent.
    pub(super) fn report_skipped_liveness(&self, result: &mut CheckResult) {
        let decided: std::collections::HashSet<&str> = result
            .violations
            .iter()
            .map(|v| v.property_name.as_str())
            .collect();
        let named: std::collections::HashSet<&str> = result
            .unverified
            .iter()
            .map(|u| u.property_name.as_str())
            .collect();
        let skipped: Vec<String> = self
            .model
            .ltl_formulas()
            .iter()
            .filter(|f| !is_safety_formula(&f.formula))
            .map(|f| f.name.as_deref().unwrap_or("ltl").to_string())
            .filter(|n| !decided.contains(n.as_str()) && !named.contains(n.as_str()))
            .collect();
        for name in skipped {
            result.unverified.push(Unverified::new(
                name,
                UnverifiedKind::EarlyStop,
                "the run stopped at the first violation, so the liveness pass was skipped and this claim was never examined - it is not satisfied",
            ));
        }
    }

    pub(super) fn check_ltl_properties(&self, result: &mut CheckResult) {
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
                            // A property that could not be checked is neither violated nor
                            // satisfied. Counting it as an error made a SATISFIED property
                            // read as invalid; treating it as clean made a false property
                            // read as verified. Report it as unverified.
                            log::warn!("LTL check error for '{}': {}", prop_name, e);
                            result.unverified.push(Unverified::new(
                                prop_name.to_string(),
                                UnverifiedKind::UnsupportedProperty,
                                format!("{}", e),
                            ));
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Failed to parse LTL formula '{}': {}", prop_name, e);
                    result.unverified.push(Unverified::new(
                        prop_name.to_string(),
                        UnverifiedKind::UnparsableFormula,
                        format!("LTL formula could not be parsed: {}", e),
                    ));
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
        let mut hit_depth_limit = false;
        let mut hit_state_limit = false;
        let mut hit_early_stop = false;
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
            if self.config.stop_at_first_violation && !violations.is_empty() {
                hit_early_stop = true;
                break;
            }
            max_depth = max_depth.max(depth);

            if depth >= self.config.max_depth {
                hit_depth_limit = true;
                continue;
            }
            if storage.len() >= self.config.max_states {
                hit_state_limit = true;
                break;
            }

            if self.config.check_assertions
                && let Some(desc) = self.model.check_violation(&state)
            {
                let state_trail = self.build_trail(&trail, state_idx);
                let name = model_violation_name(&desc);
                violations.push(Violation {
                    property_name: name.to_string(),
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
                    match evaluate_propositional(&check.formula, &state_str, &self.model, &state) {
                        Some(true) => {}
                        Some(false) => {
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
                        None => {
                            // An atom that does not resolve must never be read as false.
                            let state_trail = self.build_trail(&trail, state_idx);
                            violations.push(Violation {
                                property_name: check.prop_name.clone(),
                                trail: state_trail,
                                description: format!(
                                    "Unresolved atomic proposition in '{}': '{}' is not a variable in state {}",
                                    check.prop_name, check.formula, state_str
                                ),
                            });
                            if violations.len() >= 100 {
                                break;
                            }
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

        if transitions_count == 0
            && self.model.process_count() > 0
            && !violations.iter().any(|v| v.property_name == "model")
            // A deadlock at the initial state already explains why nothing was explored.
            // Reporting it twice inflated the error count: one blocked model produced two
            // errors where pan reports one, and the second said nothing had been verified
            // when the search had in fact decided the model.
            && !violations.iter().any(|v| v.property_name == "deadlock")
        {
            violations.push(Violation {
                property_name: "model".to_string(),
                trail: vec![],
                description: format!(
                    "no transitions explored: the model declares {} process(es) but none are enabled in the initial state, so nothing was verified",
                    self.model.process_count()
                ),
            });
        }

        let violations = dedup_violations(violations);

        let mut result = CheckResult {
            states_explored: storage.len(),
            states_stored: storage.len(),
            transitions: transitions_count,
            depth_reached: max_depth,
            errors: violations.len(),
            violations,
            unverified: Vec::new(),
            elapsed_secs: elapsed,
        };

        if hit_depth_limit {
            result.unverified.push(Unverified {
                property_name: "model".to_string(),
                kind: UnverifiedKind::DepthBudget,
                reason: format!(
                    "search pruned states at the depth limit of {} before the state space closed",
                    self.config.max_depth
                ),
                states_used: storage.len(),
                budget: None,
            });
        }
        if hit_state_limit {
            result.unverified.push(Unverified {
                property_name: "model".to_string(),
                kind: UnverifiedKind::StateBudget,
                reason: format!(
                    "search stopped at the state limit of {} before the state space closed",
                    self.config.max_states
                ),
                states_used: storage.len(),
                budget: Some(self.config.max_states),
            });
        }

        // BFS previously returned before this call, so every non-propositional LTL
        // property was silently unchecked under --search bfs while still reported as a
        // clean verification.
        self.check_ltl_properties(&mut result);

        // With an early stop, every claim that did not produce a violation was left
        // unexplored. Report them individually so a caller cannot read "no violation
        // reported" as "satisfied" - the failure mode this whole contract exists to stop.
        if hit_early_stop {
            let decided: std::collections::HashSet<&str> = result
                .violations
                .iter()
                .map(|v| v.property_name.as_str())
                .collect();
            let undecided: Vec<Unverified> = self
                .model
                .ltl_formulas()
                .iter()
                .map(|f| f.name.as_deref().unwrap_or("ltl"))
                .filter(|n| !decided.contains(*n))
                .map(|n| {
                    Unverified::new(
                        n.to_string(),
                        UnverifiedKind::EarlyStop,
                        "search stopped at the first violation, so this claim was not fully explored",
                    )
                })
                .collect();
            result.unverified.extend(undecided);
        }

        result
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
            StorageMode::HashCompact => Box::new(HashCompactStore::<M::State>::new(1024)),
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
            unverified: Vec::new(),
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
            if let Ok(crate::property::LtlFormula::Always(inner)) =
                crate::property::LtlFormula::parse(formula_str)
                && is_propositional(&inner)
            {
                checks.push(SafetyCheck {
                    prop_name,
                    formula: *inner.clone(),
                });
            }
        }

        checks
    }
}

/// Name a model-level violation so it can never be mistaken for a claim.
///
/// Callers such as veriplan attribute a violation to a requirement by reading the claim index
/// out of the property name, so a deadlock reported as "assertion" is misleading and a
/// deadlock reported as a claim name would be actively wrong.
fn model_violation_name(description: &str) -> &'static str {
    if description.starts_with("deadlock") {
        "deadlock"
    } else if description.starts_with("no transitions explored") {
        "model"
    } else {
        "assertion"
    }
}

/// A safety property to check during DFS: []expr where expr is propositional.
struct SafetyCheck {
    /// Property name for error reporting.
    prop_name: String,
    /// The propositional formula (inner of []).
    formula: crate::property::LtlFormula,
}

/// Collapse violations to one per property, keeping the first trail.
///
/// The search reaches the same property violation through many states and paths, so the
/// raw list length is a count of states hit, not a count of findings. Real spin stops at
/// the first error by default and downstream tooling reads a count as distinct findings.
pub(super) fn dedup_violations(violations: Vec<Violation>) -> Vec<Violation> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for v in violations {
        if seen.insert(v.property_name.clone()) {
            out.push(v);
        }
    }
    out
}
