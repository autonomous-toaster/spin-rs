# per-state-throughput Specification

## Purpose

Throughput is never bought with approximate storage. Any state count or verdict this engine reports comes from an exact store that compares full states, so a speed measurement can never become a completeness claim. Per-state cost itself is the subject of the `per-state-cost` capability.

## Requirements

### Requirement: Speed Is Not Bought With Approximate Storage
T4.4 SHALL report ALWAYS states per second from an exact store.

`StorageMode::HashCompact` exists and is not the default: it trades completeness for speed and cannot underwrite a state-count claim or a verdict. The veriplan performance change measured throughput and did not adopt it.

The per-state cost, and the floor that states a target for it, sits in `per-state-cost`: the floor as first written cannot be measured on the fixture it named, and the correction is recorded in the archived change's `tasks.md`.

#### Scenario: T4.4 a throughput figure is not a hash-only figure
GIVEN the corpus and the T4.4 measurement
WHEN T4.4 reports states per second
THEN the run SHALL use `StorageMode::Exact`, and a state count or verdict SHALL NOT be reported from a hash-only store.
