# Liveness Budget

## ADDED Requirements

### Requirement: No Run Is Unbounded
T3.1 SHALL complete BEFORE T3.5 SHALL run. Every traversal SHALL be bounded ALWAYS by a state ceiling, including the nested DFS used for liveness and never claims.

Today the safety traversal divides `max_states` into per-claim shares and reports `StateBudget`; the liveness traversal has no ceiling at all and no caller sets `with_max_depth`. A wide product also closes deep, so depth alone is not a substitute for a state count.

#### Scenario: T3.5 one-state budget on a liveness claim
GIVEN a ten-task chain with one `<>` claim
WHEN T3.5 runs `--max-states 1`
THEN the run SHALL terminate in under a second and report that claim in `unverified` with kind `StateBudget`.

### Requirement: Exhaustion Is Not Closure
T3.2 SHALL complete BEFORE T3.4 SHALL run. The liveness search SHALL distinguish ALWAYS an exhausted search from a closed one, and MUST NOT report an exhausted search as a satisfied claim.

`NestedDFS::check` returns `Option<Violation>`, where `None` currently means both "closed, no accepting cycle" and "did not finish". The caller cannot tell them apart, so a caller's fuse is fiction and a partial search reads as clean.

#### Scenario: T3.2 exhausted search
GIVEN a liveness claim whose product space exceeds its share
WHEN T3.2 finishes the traversal
THEN the result SHALL carry an exhausted marker, and the claim SHALL NOT be counted as satisfied.

### Requirement: Claim Attribution On Exhaustion
T3.4 SHALL complete BEFORE T3.7 SHALL run. An undecided liveness claim SHALL appear ALWAYS in `unverified` with kind `StateBudget` and its own name, preserving the checker invariant that every claim lands in `violations` or in `unverified`.

#### Scenario: T3.4 attribution
GIVEN a model with several claims where the second liveness claim exhausts its share
WHEN T3.4 reports the result
THEN that claim SHALL be named in `unverified` and the others SHALL keep their earned verdicts.

### Requirement: Budgeting Does Not Change Earned Verdicts
T3.6 SHALL complete BEFORE T3.7 SHALL run. A liveness claim that finishes within budget SHALL report ALWAYS the same verdict as before the budget existed.

#### Scenario: T3.6 closing claim unchanged
GIVEN the liveness fixtures whose product space closes within budget
WHEN T3.6 runs with and without the ceiling
THEN the verdicts SHALL be identical.

### Requirement: A Budget Below The Claim Count Is Specified, Not Clamped
T3.3 SHALL complete BEFORE T3.8 SHALL run. When `max_states` is smaller than the number of claims, the run SHALL ALWAYS apply one stated rule, report that rule in its output, and report every claim it could not decide.

The rule today is `share = max(max_states / claims, 1)` with no redistribution back, applied at src/engine/checker/por_run.rs:60.

#### Scenario: T3.8 the rule is reported
GIVEN a model with three claims and `max_states 2`
WHEN T3.8 runs
THEN the output SHALL state that the per-claim share was raised to one state, and each claim SHALL be named in `unverified` with kind `StateBudget`.

#### Scenario: T3.9 the explored total is bounded
GIVEN a model with three claims and `max_states 2`, whose floor yields one state per claim
WHEN T3.9 runs
THEN the reported total states explored SHALL NOT exceed `max_states`, or the run SHALL state that the floor raised it to three. The reported total SHALL NOT exceed the states the run actually explored.

#### Scenario: T3.8a zero budget is not silently ignored
GIVEN `max_states 0`
WHEN T3.8 runs
THEN the run SHALL either refuse the configuration with a diagnostic or report the raised budget explicitly, and MUST NOT silently explore states as though a positive budget had been given.
