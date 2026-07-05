# Hash-Compact Storage

## Task Reference

| Task ID | Description |
|---------|-------------|
| T5.1 | Add StorageMode::HashCompact variant |
| T5.2 | Implement hash-compact store |
| T5.3 | Implement collision detection |
| T5.4 | Add --hc CLI option |
| T5.5 | Test: hash-compact matches exact |

## ADDED Requirements

### Requirement: Hash-Compact Store

T5.2 SHALL complete BEFORE T5.3 SHALL run. The store SHALL maintain a hash table of 64-bit state hashes and a small LRU cache of recent full states. T5.3 SHALL detect hash collisions by comparing the full state from the LRU cache. On collision detection, the state falls back to exact storage.

#### Scenario: HC-1: Hash-compact correctness

GIVEN a model with known state space
WHEN T5.5 runs with both exact and hash-compact storage
THEN the set of violations found SHALL be identical.

### Requirement: Correctness

T5.5 SHALL complete AFTER T5.3. Hash-compact storage SHALL produce the same verification results as exact storage (same violations found, same states explored).

#### Scenario: HC-1: Hash-compact correctness

GIVEN a model with known state space
WHEN T5.5 runs with both exact and hash-compact storage
THEN the set of violations found SHALL be identical.
