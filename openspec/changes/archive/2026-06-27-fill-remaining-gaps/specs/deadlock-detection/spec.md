## Task Reference

| Task ID | Description |
|---------|-------------|
| T1.4 | Add deadlock detection to `LuaModel::check_violation` |
| T1.7 | Wire deadlock result into benchmark error count |
| T1.6 | Test: `deadlock_circular` detects exactly 1 error (deadlock) |
| T1.8 | Ensure deadlock detection respects terminated processes |

## ADDED Requirements

### Requirement: Detect deadlocks in check_violation

T1.4 SHALL ALWAYS implement `check_violation` for `LuaModel`. After transition enumeration, if `transitions.len() == 0` and at least one process is not terminated (i.e., `state._done_<name>` is false for at least one proctype), the function SHALL return `Some("deadlock")`.

#### Scenario: Circular deadlock detected

- **WHEN** T1.4 evaluates a state from `deadlock_circular` where P is blocked on `ch1 ! 1` and Q is blocked on `ch2 ! 1`
- **THEN** `check_violation` SHALL return `Some(...)` describing the deadlock

#### Scenario: Normal termination not deadlock

- **WHEN** T1.4 evaluates a state where all processes have `_done_<name> == true`
- **THEN** `check_violation` SHALL return `None`

### Requirement: Benchmark reports deadlock errors

T1.7 SHALL ALWAYS update the benchmark runner so that deadlocks detected by `spin_rs` are counted as errors, matching Spin's expected error count.

### Requirement: deadlock_circular gets 1 error

T1.6 SHALL ALWAYS verify that `deadlock_circular` produces exactly 1 error when verified. T1.6 SHALL NOT skip this model in the benchmark.

#### Scenario: Deadlock detected

- **WHEN** T1.6 runs `verify(DEADLOCK_CIRCULAR)`
- **THEN** the result SHALL have `errors == 1`
- **AND** the violation description SHALL contain "deadlock"

### Requirement: Terminated processes excluded

T1.8 SHALL ALWAYS ensure that a process which has executed `break` (is done) is not counted as blocked. Only processes with `_done_<name> == false` contribute to deadlock detection.

#### Scenario: One process done, one blocked

- **WHEN** process A is done and process B is blocked
- **THEN** it SHALL BE flagged as a deadlock (B is stuck)
