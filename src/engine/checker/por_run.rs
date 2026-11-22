use super::dfs::dedup_violations;
use super::*;

impl<M: Model> Checker<M> {
    /// Run one reduced traversal per claim, aggregated into one result.
    ///
    /// The public contract is unchanged: one call still reports every violation, attributed by
    /// claim name. Two numbers change and are documented on the fields rather than left to be
    /// discovered - states_explored is a sum over per-claim traversals, and max_states is shared
    /// out between the claims rather than given to each.
    ///
    /// ## Why the budget is shared out
    ///
    /// `max_states` used to be one pool that every traversal drew from in declaration order, so the
    /// first claim could consume all of it: a plan with sixty claims reported the first one and
    /// starved the rest, and the run still looked complete because every starved claim was named.
    /// Naming them is not the same as giving them a chance.
    ///
    /// Each claim now gets its own share, `max_states / claims`, and unused shares are not
    /// redistributed. That is a deliberate choice over rolling the leftovers forward: rolling them
    /// forward lets an early claim that needs two shares take them from later claims, which is the
    /// behaviour this replaces. The cost is that a claim needing more than its share is reported
    /// starved even when the pool has slack; the benefit is that the cost of a plan is a function
    /// of its claim count rather than of its first claim.
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
        // One share per claim, at least one state each so a very small budget still gives every
        // claim the chance to be refuted rather than silently nothing.
        let share = (self.config.max_states / scopes.len().max(1)).max(1);
        // Why the remaining claims will not be checked. "Stopped" is not one condition: a run can
        // stop because it refuted a claim, or because the budget ran out, and telling a caller
        // that a claim was skipped because of a violation when there was none is a false report.
        let mut stopped: Option<&'static str> = None;
        let mut first = true;

        for scope in &scopes {
            if let Some(reason) = stopped {
                let (kind, why) = if reason == "violation" {
                    (
                        UnverifiedKind::EarlyStop,
                        "search stopped at the first violation, so this claim was not fully explored"
                            .to_string(),
                    )
                } else {
                    (
                        UnverifiedKind::StateBudget,
                        format!(
                            "the run's state budget of {} is shared out as {share} states per claim and was used up, so this claim was not reached",
                            self.config.max_states
                        ),
                    )
                };
                result.unverified.push(Unverified {
                    property_name: scope.name.clone(),
                    kind,
                    reason: why,
                });
                continue;
            }
            // This claim's own share, never another claim's. The pool cannot be exhausted by an
            // earlier claim any more, so the only way a share is unavailable is a budget too small
            // to have one at all.
            let budget = share;
            // A claim with nothing left is reported, never skipped: absence must not be allowed
            // to read as satisfaction.
            if budget == 0 {
                result.unverified.push(Unverified {
                    property_name: scope.name.clone(),
                    kind: UnverifiedKind::StateBudget,
                    reason: format!(
                        "the run's state budget of {} leaves no share for this claim",
                        self.config.max_states
                    ),
                });
                stopped = Some("budget");
                continue;
            }
            let sub = self.check_dfs_inner(Some(scope), budget, first);
            first = false;
            result.states_explored += sub.states_explored;
            result.states_stored += sub.states_stored;
            result.transitions += sub.transitions;
            result.depth_reached = result.depth_reached.max(sub.depth_reached);
            result.violations.extend(sub.violations);
            result.unverified.extend(sub.unverified);
            if self.config.stop_at_first_violation && !result.violations.is_empty() {
                stopped = Some("violation");
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
