use super::dfs::dedup_violations;
use super::*;

impl<M: Model> Checker<M> {
    /// Run one reduced traversal per claim, aggregated into one result.
    ///
    /// The public contract is unchanged: one call still reports every violation, attributed by
    /// claim name. Two numbers change and are documented on the fields rather than left to be
    /// discovered - states_explored is a sum over per-claim traversals, and max_states bounds the
    /// whole run rather than each traversal.
    pub fn check_dfs_por(&self) -> CheckResult {
        let start = std::time::Instant::now();
        let scopes = self.model.reduction_scopes();
        let mut result = CheckResult {
            states_explored: 0,
            states_stored: 0,
            transitions: 0,
            depth_reached: 0,
            errors: 0,
            violations: Vec::new(),
            unverified: Vec::new(),
            elapsed_secs: 0.0,
        };
        let mut used = 0usize;
        let mut stopped = false;
        let mut first = true;

        for scope in &scopes {
            if stopped {
                result.unverified.push(Unverified {
                    property_name: scope.name.clone(),
                    kind: UnverifiedKind::EarlyStop,
                    reason: "search stopped at the first violation, so this claim was not fully explored".to_string(),
                });
                continue;
            }
            let budget = self.config.max_states.saturating_sub(used);
            // A claim with nothing left is reported, never skipped: absence must not be allowed
            // to read as satisfaction.
            if budget == 0 {
                result.unverified.push(Unverified {
                    property_name: scope.name.clone(),
                    kind: UnverifiedKind::StateBudget,
                    reason: format!(
                        "the run's state budget of {} was exhausted before this claim was reached; claims are checked in declaration order",
                        self.config.max_states
                    ),
                });
                stopped = true;
                continue;
            }
            let sub = self.check_dfs_inner(Some(scope), budget, first);
            first = false;
            used += sub.states_explored;
            result.states_explored += sub.states_explored;
            result.states_stored += sub.states_stored;
            result.transitions += sub.transitions;
            result.depth_reached = result.depth_reached.max(sub.depth_reached);
            result.violations.extend(sub.violations);
            result.unverified.extend(sub.unverified);
            if self.config.stop_at_first_violation && !result.violations.is_empty() {
                stopped = true;
            }
        }

        result.violations = dedup_violations(result.violations);
        result.errors = result.violations.len();
        result.elapsed_secs = start.elapsed().as_secs_f64();

        // Liveness claims live outside the per-claim safety loop. The nested DFS is never
        // reduced, because an accepting cycle can consist of invisible transitions.
        if !(self.config.stop_at_first_violation && result.errors > 0) {
            self.check_ltl_properties(&mut result);
        }
        result
    }
}
