# Statement Merging

## Task Reference

| Task ID | Description |
|---------|-------------|
| T3.1 | Implement mergeability check |
| T3.2 | Implement merge pass |
| T3.3 | Handle merged guard |
| T3.4 | Handle merged effect |
| T3.5 | Add -o3 CLI flag |
| T3.6 | Test: merged transitions correct |
| T3.7 | Test: non-mergeable not merged |

## ADDED Requirements

### Requirement: Mergeability

T3.1 SHALL complete BEFORE T3.2 SHALL run. Two consecutive transitions are mergeable when both are deterministic, neither blocks, and neither is a channel operation. T3.2 SHALL combine consecutive mergeable transitions into single transitions.

#### Scenario: Mergeability scenario

GIVEN the requirement is implemented
WHEN the system runs
THEN the behavior SHALL be correct

### Requirement: Merged Semantics

T3.3 SHALL complete AFTER T3.2. The merged guard SHALL be the AND of both guards. T3.4 SHALL make the merged effect the sequence of both effects.

#### Scenario: MERGE-1: Merged transitions correct

GIVEN a proctype with `x = 1; y = 2; z = x + y`
WHEN T3.6 runs with `-o3`
THEN the three assignments SHALL be merged into fewer transitions, and the final state SHALL have x=1, y=2, z=3.
