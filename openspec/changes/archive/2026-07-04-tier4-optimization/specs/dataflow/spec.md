# Dataflow Analysis
## Task Reference

| Task ID | Description |
|---------|-------------|
| T1.1 | Design GEN/KILL set computation |
| T1.2 | Implement GEN set |
| T1.3 | Implement KILL set |
| T1.4 | Implement fixed-point iteration |
| T1.5 | Compute IN/OUT sets |
| T1.6 | Test: GEN/KILL for simple model |
| T1.7 | Test: dataflow handles loops |


## ADDED Requirements

### Requirement: GEN/KILL Sets
T1.1 SHALL complete BEFORE T1.2 SHALL run. Each transition SHALL have a GEN set (variables read by guard) and a KILL set (variables written by effect). T1.2 SHALL extract variable reads from guard expressions. T1.3 SHALL extract variable writes from effect expressions.

#### Scenario: GEN/KILL Sets scenario

GIVEN the requirement is implemented
WHEN the system runs
THEN the behavior SHALL be correct

### Requirement: Fixed-Point Iteration
T1.4 SHALL complete BEFORE T1.5 SHALL run. IN/OUT sets SHALL be computed by iterating over the control-flow graph until convergence. T1.5 SHALL compute IN[n] = union of OUT[p] for all predecessors p, and OUT[n] = (IN[n] - KILL[n]) union GEN[n].

#### Scenario: Fixed-Point Iteration scenario

GIVEN the requirement is implemented
WHEN the system runs
THEN the behavior SHALL be correct

## Scenarios

### DATAFLOW-1: Simple model
GIVEN a proctype with `x = 1; y = x + 1`
WHEN T1.6 runs
THEN GEN of first transition SHALL be empty, KILL SHALL contain {x}. GEN of second transition SHALL contain {x}, KILL SHALL contain {y}.
