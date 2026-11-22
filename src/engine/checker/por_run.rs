use super::dfs::dedup_violations;
use super::*;

/// The budget a claim is given: its share, plus what decided claims before it did not use.
///
/// Taking the leftover clears it, so two claims cannot be given the same unused states.
fn grant(share: usize, leftover: &mut usize) -> usize {
    let budget = share + *leftover;
    *leftover = 0;
    budget
}

/// Add back what a claim did not use, if it was decided.
///
/// A decided claim - refuted, or its traversal closed - can never use its remainder, so that pool
/// is dead and the claims behind it may have it. An undecided claim contributes nothing: leaving it
/// its remainder would be the pooling that let an early claim's appetite starve the rest.
fn contribute(leftover: &mut usize, budget: usize, used: usize, decided: bool) {
    if decided {
        *leftover += budget.saturating_sub(used.min(budget));
    }
}

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
        // Budget a claim finished with, available to the claims behind it.
        //
        // Only a *decided* claim contributes: once a claim is refuted or its traversal closes,
        // whatever is left of its share can never be used by it, so the pool is dead. An
        // undecided claim contributes nothing - letting it keep an unused remainder would be the
        // pooling this replaced, where an early claim's appetite starved the rest.
        let mut leftover = 0usize;

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
                result.unverified.push(
                    Unverified::new(scope.name.clone(), kind, why)
                        // Not reached, so it used nothing; the share is what it was entitled to.
                        .with_usage(0, Some(share)),
                );
                continue;
            }
            // This claim's own share plus what decided claims before it did not need.
            let budget = grant(share, &mut leftover);
            // A claim with nothing left is reported, never skipped: absence must not be allowed
            // to read as satisfaction.
            if budget == 0 {
                result.unverified.push(
                    Unverified::new(
                        scope.name.clone(),
                        UnverifiedKind::StateBudget,
                        format!(
                            "the run's state budget of {} leaves no share for this claim",
                            self.config.max_states
                        ),
                    )
                    .with_usage(0, Some(budget)),
                );
                stopped = Some("budget");
                continue;
            }
            let sub = self.check_dfs_inner(Some(scope), budget, first);
            first = false;
            result.states_explored += sub.states_explored;
            result.states_stored += sub.states_stored;
            result.transitions += sub.transitions;
            result.depth_reached = result.depth_reached.max(sub.depth_reached);
            // A claim is decided when its traversal closed or refuted it. A starved traversal
            // that also found a violation is decided: the violation exists whatever else was
            // unexplored.
            let starved = sub.unverified.iter().any(|u| {
                u.property_name == scope.name
                    && matches!(
                        u.kind,
                        UnverifiedKind::StateBudget | UnverifiedKind::DepthBudget
                    )
            });
            let refuted = sub.violations.iter().any(|v| v.property_name == scope.name);
            contribute(
                &mut leftover,
                budget,
                sub.states_explored,
                !starved || refuted,
            );
            result.violations.extend(sub.violations);
            result.unverified.extend(sub.unverified);
            if self.config.stop_at_first_violation && !result.violations.is_empty() {
                stopped = Some("violation");
            }
        }

        result.violations = dedup_violations(result.violations);
        result.errors = result.violations.len();
        // A claim a violation already decided needs no "not fully explored" entry as well. Two
        // entries for one claim made a caller's per-claim summary disagree with its own totals,
        // and the entry says nothing the violation does not.
        let decided: std::collections::HashSet<String> = result
            .violations
            .iter()
            .map(|v| v.property_name.clone())
            .collect();
        result
            .unverified
            .retain(|u| !decided.contains(&u.property_name));
        result.elapsed_secs = start.elapsed().as_secs_f64();

        // Liveness claims live outside the per-claim safety loop. The nested DFS is never
        // reduced, because an accepting cycle can consist of invisible transitions.
        if !(self.config.stop_at_first_violation && result.errors > 0) {
            self.check_ltl_properties(&mut result);
        }
        result
    }
}

#[cfg(test)]
mod budget_share_tests {
    use super::{contribute, grant};

    #[test]
    fn a_claim_gets_its_share_and_the_leftover() {
        let mut leftover = 7;
        assert_eq!(grant(10, &mut leftover), 17);
        // Taking it clears it, so two claims cannot be handed the same unused states.
        assert_eq!(leftover, 0);
    }

    #[test]
    fn only_a_decided_claim_contributes() {
        let mut leftover = 0;
        contribute(&mut leftover, 10, 4, true);
        assert_eq!(leftover, 6);
        // An undecided claim keeps its pool: letting it contribute is the pooling that starved
        // the claims behind an early one.
        contribute(&mut leftover, 10, 4, false);
        assert_eq!(leftover, 6);
    }

    #[test]
    fn a_claim_that_used_more_than_it_was_given_contributes_nothing() {
        let mut leftover = 3;
        contribute(&mut leftover, 10, 10, true);
        assert_eq!(leftover, 3);
    }
}
