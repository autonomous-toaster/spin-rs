# differential-conformance Specification

## Purpose
TBD - created by archiving change checker-soundness. Update Purpose after archive.

## Requirements

### Requirement: Corpus Covers Nested Shapes
T4.1 SHALL complete BEFORE T4.2 SHALL run. The differential corpus SHALL include ALWAYS the m1, m2, m3 shapes and the veriplan generated `if/fi` shape, so that a nested-control-flow regression is detected.

#### Scenario: T4.1 regression detection
GIVEN a regression that drops a proctype containing `if/fi`
WHEN T4.1 runs the corpus
THEN at least one model SHALL report a state-count mismatch against real spin.

### Requirement: Counting Comparison
T4.2 SHALL complete AFTER T4.1. Verdict comparison SHALL compare ALWAYS per-property error counts and explored state counts, and SHALL NOT rely on whether any error was reported.

#### Scenario: T4.2 count mismatch
GIVEN a model where spin-rs reports 61 errors for one property and real spin reports none
WHEN T4.2 runs
THEN the comparison SHALL fail, reporting both counts.
