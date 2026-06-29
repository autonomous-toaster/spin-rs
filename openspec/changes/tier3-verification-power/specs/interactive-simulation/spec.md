# Interactive Simulation
## Task Reference

| Task ID | Description |
|---------|-------------|
| T1.1 | Create InteractiveSimulator struct |
| T1.2 | Implement step() with user choice |
| T1.3 | Implement step-back with history |
| T1.4 | Implement state inspection |
| T1.5 | Add --interactive CLI flag |
| T1.6 | Test: interactive simulation path |


## ADDED Requirements

### Requirement: InteractiveSimulator
T1.1 SHALL complete BEFORE T1.2 SHALL run. The InteractiveSimulator SHALL wrap a model and provide step-by-step control. T1.2 SHALL display the current state, list enabled transitions, and read the user's choice from stdin.

#### Scenario: InteractiveSimulator scenario

GIVEN the requirement is implemented
WHEN the system runs
THEN the behavior SHALL be correct

### Requirement: Step-Back
T1.3 SHALL complete AFTER T1.2. The simulator SHALL store a history of previous states. The user SHALL be able to undo the last step and return to the previous state.

#### Scenario: Step-Back scenario

GIVEN the requirement is implemented
WHEN the system runs
THEN the behavior SHALL be correct

### Requirement: State Inspection
T1.4 SHALL complete AFTER T1.2. The user SHALL be able to inspect variable values at the current step. All global and local variables SHALL be displayed.

#### Scenario: State Inspection scenario

GIVEN the requirement is implemented
WHEN the system runs
THEN the behavior SHALL be correct

## Scenarios

### INTERACTIVE-1: Interactive simulation path
GIVEN a model with non-deterministic choice
WHEN T1.6 runs with user selecting specific transitions
THEN the simulation SHALL follow the user's chosen path.
