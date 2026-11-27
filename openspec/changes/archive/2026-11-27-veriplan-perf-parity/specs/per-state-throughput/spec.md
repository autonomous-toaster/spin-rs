# Per State Throughput

## ADDED Requirements

### Requirement: Speed Is Not Bought With Approximate Storage

T4.4 SHALL report ALWAYS states per second from an exact store.

`StorageMode::HashCompact` exists and is not the default: it trades completeness for speed and cannot underwrite a state-count claim or a verdict. This change measured throughput and did not adopt it.

The per-state cost, and the floor that would state a target for it, sits in `openspec/changes/per-state-cost`. The floor as first written here cannot be measured on the fixture it names; the correction is in `tasks.md`.

#### Scenario: T4.4 a throughput figure is not a hash-only figure

GIVEN the corpus and the T4.4 measurement
WHEN T4.4 reports states per second
THEN the run SHALL use `StorageMode::Exact`, and a state count or verdict SHALL NOT be reported from a hash-only store.
