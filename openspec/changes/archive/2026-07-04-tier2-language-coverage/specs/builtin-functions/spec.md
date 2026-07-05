# Built-in Functions

## Task Reference

| Task ID | Description |
|---------|-------------|
| T3.1-T3.7 | Add parsing for each built-in function |
| T3.8-T3.12 | Implement FFI for each built-in function |
| T3.13 | Update codegen to emit FFI calls |
| T3.14-T3.16 | Tests |

## ADDED Requirements

### Requirement: Parsing

T3.1 SHALL complete BEFORE T3.13 SHALL run. `enabled(expr)` SHALL be parsed as `Expression::FuncCall { name: "enabled", args: [expr] }`. Similarly for all other built-in functions.

#### Scenario: Parsing scenario

GIVEN the requirement is implemented
WHEN the system runs
THEN the behavior SHALL be correct

### Requirement: FFI Implementation

T3.8 SHALL complete BEFORE T3.13 SHALL run. `_spin_enabled(pid)` returns true when the process with the given pid is runnable (has at least one enabled transition). T3.9 `_spin_timeout()` returns true when no process can make progress. T3.10 `_spin_np_()` returns true when the current state has no progress label visited.

#### Scenario: FFI Implementation scenario

GIVEN the requirement is implemented
WHEN the system runs
THEN the behavior SHALL be correct

### Requirement: Codegen

T3.13 SHALL complete BEFORE T3.14 SHALL run. The codegen SHALL emit `_spin_enabled(pid)` for `enabled(pid)`, `_spin_timeout()` for `timeout`, etc.

#### Scenario: Codegen scenario

GIVEN the requirement is implemented
WHEN the system runs
THEN the behavior SHALL be correct

## Scenarios

### BUILTIN-1: enabled() in guard

GIVEN a guard `enabled(1)` where process pid 1 is blocked
WHEN T3.14 runs
THEN the guard SHALL evaluate to false.

### BUILTIN-2: timeout in never claim

GIVEN a never claim that uses `timeout` as a guard
WHEN T3.15 runs and no process can make progress
THEN the timeout guard SHALL evaluate to true.
