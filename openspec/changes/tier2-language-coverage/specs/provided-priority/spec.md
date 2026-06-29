# Provided / Priority
## Task Reference

| Task ID | Description |
|---------|-------------|
| T6.1 | Add parsing for provided clause |
| T6.2 | Add parsing for priority clause |
| T6.3 | AND provided clause with every transition guard |
| T6.4 | Store priority in state vector |
| T6.5 | Implement priority-based scheduling |
| T6.6-T6.7 | Tests |


## ADDED Requirements

### Requirement: Provided Clause
T6.1 SHALL complete BEFORE T6.3 SHALL run. `active proctype P() provided (x > 0)` SHALL be parsed. T6.3 SHALL AND the provided condition with every transition guard of process P.

#### Scenario: PROVIDED-1: Provided clause prevents execution
GIVEN `active proctype P() provided (x > 0)` where x is initially 0
WHEN T6.6 runs
THEN no transition of P SHALL be enabled until x becomes > 0.

### Requirement: Priority Clause
T6.2 SHALL complete BEFORE T6.4 SHALL run. `active proctype P() priority 3` SHALL be parsed. T6.4 SHALL store the priority in the state vector. T6.5 SHALL use priority to order process scheduling: higher priority processes SHALL be explored before lower priority ones.

#### Scenario: PROVIDED-1: Provided clause prevents execution
GIVEN `active proctype P() provided (x > 0)` where x is initially 0
WHEN T6.6 runs
THEN no transition of P SHALL be enabled until x becomes > 0.

#### Scenario: PRIORITY-1: Priority affects scheduling
GIVEN two active proctypes: `P() priority 3` and `Q() priority 1`
WHEN T6.7 runs
THEN P's transitions SHALL be explored before Q's transitions in the initial state.
