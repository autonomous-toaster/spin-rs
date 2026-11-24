# Per State Throughput

## ADDED Requirements

### Requirement: Throughput Floor
T4.1 SHALL complete BEFORE T4.4 SHALL run. The engine SHALL sustain ALWAYS at least 1e5 states per second on the consumer's chain shape in a release build, measured on the committed fixture.

Baseline at v0.5.35: 2806 states in 0.412 s = ~6.8k states/s, against pan's 1e5-1e6. An order of magnitude is the floor because at 6.8k states/s a 24-task plan is unreachable even after the state count is linear.

#### Scenario: T4.5 floor holds
GIVEN the chain fixture at N=8 in a release build
WHEN T4.5 measures states per second over three runs
THEN the median SHALL be at least 1e5 states/s.

### Requirement: Throughput Work Does Not Change Verdicts
T4.1 SHALL complete BEFORE T4.5 SHALL run. The throughput changes SHALL preserve ALWAYS every verdict in the corpus, and MUST NOT be achieved by storing hashes instead of states.

`StorageMode::HashCompact` exists and is not the default. It trades completeness for speed and cannot be the basis of a state-count or verdict claim.

#### Scenario: T4.3 structured state
GIVEN the corpus
WHEN T4.3 replaces the owned string state with a structured value
THEN every verdict SHALL be unchanged and the store SHALL still compare full states, not hashes only.
