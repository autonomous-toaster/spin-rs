# Remote References

## Task Reference

| Task ID | Description |
|---------|-------------|
| T4.1-T4.4 | Add parsing for remote reference syntax |
| T4.5-T4.6 | Implement FFI for remote references |
| T4.7 | Update codegen to emit FFI calls |
| T4.8-T4.9 | Tests |

## ADDED Requirements

### Requirement: Remote Label Reference

T4.1 SHALL complete BEFORE T4.5 SHALL run. `P[pid]@label` SHALL be parsed as a remote reference to process P's label. T4.5 `_spin_remote_label(pid, label_step)` returns true when process pid's step variable equals label_step.

#### Scenario: REMOTE-1: Remote label in guard

GIVEN a guard `P[1]@ready` where process P with pid 1 is at label `ready`
WHEN T4.8 runs
THEN the guard SHALL evaluate to true.

#### Scenario: REMOTE-2: Remote variable read

GIVEN `P[1]:counter` where process P with pid 1 has local variable `counter = 5`
WHEN T4.9 runs
THEN the expression SHALL evaluate to 5.

### Requirement: Remote Variable Reference

T4.2 SHALL complete BEFORE T4.6 SHALL run. `P[pid]:var` SHALL be parsed as a remote reference to process P's local variable. T4.6 `_spin_remote_var(pid, var_name)` SHALL return the value of process pid's local variable.

#### Scenario: REMOTE-1: Remote label in guard

GIVEN a guard `P[1]@ready` where process P with pid 1 is at label `ready`
WHEN T4.8 runs
THEN the guard SHALL evaluate to true.

#### Scenario: REMOTE-2: Remote variable read

GIVEN `P[1]:counter` where process P with pid 1 has local variable `counter = 5`
WHEN T4.9 runs
THEN the expression SHALL evaluate to 5.
