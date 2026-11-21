# Verdict Integrity
## Task Reference

| Task ID | Description |
|---------|-------------|
| T2.1 | Error when a model has proctypes but zero transitions |
| T2.2 | Surface Lua evaluation failures |
| T2.3 | Deduplicate violations per property |
| T4.2 | Compare per-property error counts |

## ADDED Requirements

### Requirement: No Success Without Exploration
T2.1 SHALL complete BEFORE T4.2 SHALL run. The verifier SHALL report an error ALWAYS when a model declares at least one proctype and the search explores zero transitions.

#### Scenario: T2.1 unexplored model
GIVEN a model with one active proctype whose transition enumeration yields nothing
WHEN T2.1 runs
THEN the result SHALL contain an error and SHALL NOT report success.

### Requirement: Evaluation Failures Are Errors
T2.2 SHALL complete BEFORE T4.2 SHALL run. A failed guard or effect evaluation SHALL be reported as a verification error ALWAYS, and SHALL NOT be mapped to an empty transition list.

#### Scenario: T2.2 runtime failure
GIVEN a generated guard that references an undefined name
WHEN T2.2 runs the search
THEN the result SHALL contain an error naming the failure.

### Requirement: Per-Property Deduplication
T2.3 SHALL complete BEFORE T4.2 SHALL run. Violations SHALL be deduplicated per property ALWAYS, so that one reachable violating state does not report as many errors.

#### Scenario: T2.3 repeated violation
GIVEN one property violated in a single reachable state reachable by many paths
WHEN T2.3 runs
THEN the reported error count for that property SHALL be one.
