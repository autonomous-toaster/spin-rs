# Rendezvous Optimization
## Task Reference

| Task ID | Description |
|---------|-------------|
| T4.1 | Detect sync channels |
| T4.2 | Detect send/recv pairs |
| T4.3 | Merge sync send/recv pair |
| T4.4 | Add -o4 CLI flag |
| T4.5 | Test: rendezvous reduces states |
| T4.6 | Test: async channels not affected |


## ADDED Requirements

### Requirement: Sync Channel Detection
T4.1 SHALL complete BEFORE T4.2 SHALL run. Channels with capacity 0 SHALL be identified as sync channels during codegen.

#### Scenario: Sync Channel Detection scenario

GIVEN the requirement is implemented
WHEN the system runs
THEN the behavior SHALL be correct

### Requirement: Send/Recv Pair Detection
T4.2 SHALL complete BEFORE T4.3 SHALL run. A send on a sync channel followed by a receive on the same channel SHALL be detected as a rendezvous pair. T4.3 SHALL merge the pair into a single transition.

#### Scenario: Send/Recv Pair Detection scenario

GIVEN the requirement is implemented
WHEN the system runs
THEN the behavior SHALL be correct

## Scenarios

### RENDEZVOUS-1: Rendezvous reduces states
GIVEN a model with `chan ch = [0] of { byte }` and a sender/receiver pair
WHEN T4.5 runs with `-o4`
THEN the number of intermediate states SHALL be reduced compared to running without `-o4`.
