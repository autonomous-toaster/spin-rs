# Non-Progress Cycles

## Task Reference

| Task ID | Description |
|---------|-------------|
| T6.1 | Add `_progress` bit to state vector |
| T6.2 | Track progress labels during codegen |
| T6.3 | Set `_progress` bit when transition visits a progress label |
| T6.4 | Implement non-progress cycle detection |
| T6.5 | Test: model with progress labels and non-progress cycle |
| T6.6 | Test: model with progress labels and no non-progress cycle |

## ADDED Requirements

### Requirement: Progress Tracking

T6.1 SHALL complete BEFORE T6.2 SHALL run. The state vector SHALL include a `_progress` bit that indicates whether a progress label has been visited on the current path.

#### Scenario: Progress Tracking scenario

GIVEN the requirement is implemented
WHEN the system runs
THEN the behavior SHALL be correct

### Requirement: Progress Label Detection

T6.2 SHALL complete BEFORE T6.3 SHALL run. Labels named `progress` SHALL be detected during codegen. T6.3 SHALL set the `_progress` bit when a transition visits a progress label.

#### Scenario: Progress Label Detection scenario

GIVEN the requirement is implemented
WHEN the system runs
THEN the behavior SHALL be correct

### Requirement: Non-Progress Cycle Detection

T6.4 SHALL complete AFTER T6.3. A second DFS SHALL detect cycles where no progress label is visited. When a cycle is found with `_progress == false`, a non-progress cycle violation SHALL be reported.

#### Scenario: NP-1: Non-progress cycle detected

GIVEN a model with a progress label and a cycle that avoids it
WHEN T6.5 runs
THEN a non-progress cycle violation SHALL be reported.

#### Scenario: NP-2: No non-progress cycle

GIVEN a model where all cycles visit a progress label
WHEN T6.6 runs
THEN no non-progress cycle violation SHALL be reported.
