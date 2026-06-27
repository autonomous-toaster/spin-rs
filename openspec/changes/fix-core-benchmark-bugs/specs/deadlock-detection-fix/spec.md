## ADDED Requirements

### Task Reference

| Task ID | Description |
|---------|-------------|
| T2.1 | Modify `check_violation` to count undoned processes individually |
| T2.2 | Only flag deadlock when undoned processes exist with zero transitions |
| T2.3 | Verify plan_5tasks_3ltls reports 0 errors after fix |
| T2.4 | Verify deadlock_circular still reports 1 error after fix |

### Requirement: Per-process done tracking

T2.1 SHALL ALWAYS parse `_done_<name>` flags from the state blob. Instead of checking for the presence of `:false` and `_done_` anywhere in the blob, T2.1 SHALL count the number of processes where `_done_<name>` is explicitly `false`. Only processes with `_done_<name> == false` SHALL contribute to deadlock detection.

#### Scenario: Two processes, one finished

- **WHEN** T2.1 evaluates a state where process A has `_done_A = true` and process B has `_done_B = false`
- **THEN** only process B SHALL BE counted as "still running" (count = 1)

### Requirement: Deadlock predicate refined

T2.2 SHALL ALWAYS flag deadlock only when ALL of the following hold:

- At least one process has `_done_<name> == false`
- That process has zero enabled transitions
- At least one such process exists (not all processes are done)

#### Scenario: Sequential completion not deadlock

- **WHEN** T2.2 evaluates a state where process A is done and process B is at end state (zero transitions, `_done_B` is `true` or B has `_nr_pr` tracking showing it terminated)
- **THEN** `check_violation` SHALL return `None`

#### Scenario: True deadlock still detected

- **WHEN** T2.2 evaluates a state from `deadlock_circular` where both P and Q are blocked (`_done_P == false && _done_Q == false` and zero transitions)
- **THEN** `check_violation` SHALL return `Some("deadlock: ...")`

### Requirement: plan benchmark models report 0 errors

T2.3 SHALL ALWAYS verify that `plan_5tasks_3ltls` and `plan_20tasks_10ltls` produce 0 errors after the deadlock fix.

#### Scenario: plan model no false deadlock

- **WHEN** T2.3 runs `verify(PLAN_5TASKS_3LTLS)` after the deadlock fix
- **THEN** `result.errors` SHALL equal 0

### Requirement: deadlock_circular still correct

T2.4 SHALL ALWAYS verify that `deadlock_circular` still detects exactly 1 error after the deadlock fix.

#### Scenario: deadlock still found

- **WHEN** T2.4 runs `verify(DEADLOCK_CIRCULAR)` after the fix
- **THEN** `result.errors` SHALL equal 1
