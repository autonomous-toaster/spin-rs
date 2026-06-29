# Parallel BFS
## Task Reference

| Task ID | Description |
|---------|-------------|
| T4.1 | Add crossbeam and dashmap dependencies |
| T4.2 | Implement concurrent BFS frontier |
| T4.3 | Implement shared visited set |
| T4.4 | Implement parallel BFS worker |
| T4.5 | Add --bfspar CLI option |
| T4.6 | Test: parallel BFS matches sequential BFS |


## ADDED Requirements

### Requirement: Concurrent Frontier
T4.2 SHALL complete BEFORE T4.4 SHALL run. The BFS frontier SHALL be a concurrent queue (crossbeam channel). Workers SHALL pop states from the frontier and push new states.

#### Scenario: Concurrent Frontier scenario

GIVEN the requirement is implemented
WHEN the system runs
THEN the behavior SHALL be correct

### Requirement: Shared Visited Set
T4.3 SHALL complete BEFORE T4.4 SHALL run. The visited set SHALL use a concurrent hashmap (dashmap) with fine-grained locking per bucket.

#### Scenario: Shared Visited Set scenario

GIVEN the requirement is implemented
WHEN the system runs
THEN the behavior SHALL be correct

### Requirement: Correctness
T4.6 SHALL complete AFTER T4.4. Parallel BFS SHALL explore the same set of states as sequential BFS, just in a different order.

#### Scenario: PBFS-1: Parallel BFS correctness
GIVEN a model with known state space
WHEN T4.6 runs with both sequential and parallel BFS
THEN the set of visited states SHALL be identical.
