# Dead Variable Elimination

## Task Reference

| Task ID | Description |
|---------|-------------|
| T2.1 | Implement liveness analysis |
| T2.2 | Mark variables as dead |
| T2.3 | Remove dead variables from state vector |
| T2.4 | Skip dead variable assignments |
| T2.5 | Add -o2 CLI flag |
| T2.6 | Test: dead variable excluded |
| T2.7 | Test: live variables not affected |

## ADDED Requirements

### Requirement: Liveness Analysis

T2.1 SHALL complete BEFORE T2.2 SHALL run. A variable is live at a program point when it is read after that point before being written. T2.2 SHALL mark variables as dead when they are written but never subsequently read.

#### Scenario: Liveness Analysis scenario

GIVEN the requirement is implemented
WHEN the system runs
THEN the behavior SHALL be correct

### Requirement: State Vector Reduction

T2.3 SHALL complete AFTER T2.2. Dead variables SHALL be excluded from the state vector initialization. T2.4 SHALL skip assignments to dead variables in the generated Lua.

#### Scenario: State Vector Reduction scenario

GIVEN the requirement is implemented
WHEN the system runs
THEN the behavior SHALL be correct

## Scenarios

### DVE-1: Dead variable excluded

GIVEN a proctype with `x = 1; y = 2; z = x + y` where z is never read
WHEN T2.6 runs with `-o2`
THEN z SHALL NOT appear in the state vector.
