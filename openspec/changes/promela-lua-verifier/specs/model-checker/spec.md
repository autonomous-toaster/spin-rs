## Task Reference

| Task ID | Description |
|---------|-------------|
| T4.1 | Implement DFS state space exploration |
| T4.2 | Implement BFS state space exploration |
| T4.3 | Implement hash-based state matching |
| T4.4 | Implement bitstate hashing storage |
| T4.5 | Implement collapse compression storage |
| T4.6 | Integrate with Lua runtime for model-specific transition calls |

## ADDED Requirements

### Requirement: DFS state exploration

T4.1 SHALL ALWAYS detect invalid end states (deadlocks) during depth-first exploration. T4.1 SHALL complete BEFORE T4.6 SHALL use the Lua runtime for transition calls. T4.1 SHALL maintain a stack of current paths for error trail generation.

#### Scenario: Simple protocol, no error

- **WHEN** T4.1 runs on a two-process mutual exclusion model
- **THEN** T4.1 SHALL explore all reachable states and report no errors

#### Scenario: Deadlock detection

- **WHEN** T4.1 runs on a model with circular channel blocking
- **THEN** T4.1 SHALL detect the invalid end state (deadlock) and generate an error trail

### Requirement: BFS state exploration

T4.2 SHALL provide breadth-first search exploration WITH the shortest counterexample property. T4.2 SHALL complete AFTER T4.3 SHALL implement hash-based state matching.

#### Scenario: Shortest counterexample

- **WHEN** T4.2 runs on a model with a violation reachable at depth 3 and depth 7
- **THEN** T4.2 SHALL report the depth-3 counterexample

### Requirement: Hash-based state matching

T4.3 SHALL ALWAYS deduplicate visited states using hash-based matching. T4.3 SHALL complete BEFORE T4.1 SHALL run DFS and BEFORE T4.2 SHALL run BFS.

#### Scenario: State deduplication

- **WHEN** T4.3 encounters the same state twice during exploration
- **THEN** T4.3 SHALL not revisit it (return `Seen` instead of `New`)

### Requirement: Bitstate hashing

T4.4 SHALL provide an approximate storage mode USING a fixed-size bit array (Bloom filter). T4.4 SHALL use two independent hash functions to index the bit array. T4.4 SHALL ALWAYS trade completeness for memory efficiency — some states may be missed (false positives).

#### Scenario: Bitstate mode

- **WHEN** T4.4 runs with `-bitstate` and a 256MB hash table
- **THEN** T4.4 SHALL use only the bit array for state matching, never storing full state vectors

### Requirement: Collapse compression

T4.5 SHALL ALWAYS compress state vectors using Spin's collapse algorithm. Canonical representation of sub-components (per-process state, per-channel state, globals) SHALL be stored in sorted hash tables and referenced by ordinals.

#### Scenario: Collapse compression

- **WHEN** T4.5 runs on a model with repeated process states
- **THEN** T4.5 SHALL store each unique process state once, referencing it by ordinal in the global state

### Requirement: Engine-runtime integration

T4.6 SHALL call the Lua runtime to enumerate transitions and compute next states during exploration. T4.6 SHALL pass the current state as a Lua value and receive a list of (guard, effect) pairs. T4.6 SHALL complete AFTER T3.2 SHALL execute transitions and BEFORE exploration begins.

#### Scenario: Interleaving exploration

- **WHEN** T4.6 explores a state with two proctypes, each having one enabled transition
- **THEN** T4.6 SHALL produce two successor states (one per proctype executing its transition)
