# Acceptance Cycles
## Task Reference

| Task ID | Description |
|---------|-------------|
| T5.1 | Track accepting states in Büchi automaton |
| T5.2 | Implement second DFS for acceptance cycles |
| T5.3 | Detect cycle when second DFS finds state on stack |
| T5.4 | Report acceptance cycle violation with trail |
| T5.5 | Test: `[]<>p` detects violation |
| T5.6 | Test: `<>[]p` detects violation |
| T5.7 | Test: liveness property holds |


## ADDED Requirements

### Requirement: Accepting State Tracking
T5.1 SHALL complete BEFORE T5.2 SHALL run. The Büchi automaton produced by ltl2ba SHALL mark which states are accepting. These marks SHALL be available to the nested DFS algorithm.

#### Scenario: Accepting State Tracking scenario

GIVEN the requirement is implemented
WHEN the system runs
THEN the behavior SHALL be correct

### Requirement: Second DFS
T5.2 SHALL complete BEFORE T5.3 SHALL run. For each accepting state found during the first DFS, a second DFS SHALL start that only follows paths staying within accepting states. T5.3 SHALL detect a cycle when the second DFS encounters a state already on its DFS stack.

#### Scenario: Second DFS scenario

GIVEN the requirement is implemented
WHEN the system runs
THEN the behavior SHALL be correct

### Requirement: Violation Reporting
T5.4 SHALL complete AFTER T5.3. When an acceptance cycle is detected, a violation SHALL be reported with a trail showing the cycle.

#### Scenario: LIVENESS-1: `[]<>p` violation
GIVEN a model where `p` eventually stops holding
WHEN T5.5 runs
THEN the nested DFS SHALL detect an acceptance cycle and report a violation.

## Scenarios

### LIVENESS-2: `<>[]p` holds
GIVEN a model where `p` eventually holds forever
WHEN T5.7 runs
THEN the nested DFS SHALL report no violation.
