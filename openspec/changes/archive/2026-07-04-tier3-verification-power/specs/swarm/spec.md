# Swarm Verification
## Task Reference

| Task ID | Description |
|---------|-------------|
| T3.1 | Design swarm config generation |
| T3.2 | Implement swarm runner |
| T3.3 | Implement result merging |
| T3.4 | Add --swarm CLI option |
| T3.5 | Test: swarm finds violation |


## ADDED Requirements

### Requirement: Swarm Config Generation
T3.1 SHALL complete BEFORE T3.2 SHALL run. N configs SHALL be generated with varied random seeds, hash functions, and search parameters. Each config SHALL differ in at least one parameter.

#### Scenario: SWARM-1: Swarm finds violation
GIVEN a model with a deep violation that a single run might miss
WHEN T3.5 runs with `--swarm 10,1`
THEN at least one worker SHALL find the violation.

### Requirement: Swarm Runner
T3.2 SHALL complete BEFORE T3.3 SHALL run. N parallel workers SHALL be spawned via rayon. Each worker SHALL run a full verification with its config. T3.3 SHALL merge results: the first violation found across all workers SHALL be reported.

#### Scenario: SWARM-1: Swarm finds violation
GIVEN a model with a deep violation that a single run might miss
WHEN T3.5 runs with `--swarm 10,1`
THEN at least one worker SHALL find the violation.
