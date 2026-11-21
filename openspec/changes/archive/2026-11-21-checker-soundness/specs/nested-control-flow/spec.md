# Nested Control Flow
## Task Reference

| Task ID | Description |
|---------|-------------|
| T1.1 | Accept optional trailing semicolon after fi and od |
| T1.2 | Reject trailing unparsed input in parse() |
| T1.3 | Classify keyword-led statements as statements in guard_body |
| T1.4 | Regression test for if/fi nested in do/od |

## ADDED Requirements

### Requirement: Terminated Block Statements
T1.1 SHALL complete BEFORE T1.4 SHALL run. The parser SHALL accept an optional trailing semicolon after `fi` and `od` ALWAYS, matching every other statement parser in the grammar.

#### Scenario: T1.1 nested if with semicolon
GIVEN a proctype whose body contains `do :: (1) -> if :: a1 = 0; fi; break od`
WHEN T1.1 runs and the model is parsed
THEN the proctype SHALL appear in the model and SHALL explore more than one state.

### Requirement: No Silent Truncation
T1.2 SHALL complete BEFORE T1.4 SHALL run. The parser SHALL return an error WHENEVER input remains unparsed after the top-level list, ALWAYS, so that a partially parsed model is never verified.

#### Scenario: T1.2 unparsable tail
GIVEN a source file whose trailing text cannot be parsed as a top-level declaration
WHEN T1.2 runs
THEN parsing SHALL fail with a diagnostic naming the unconsumed input.

### Requirement: Guard Body Classification
T1.3 SHALL complete CONCURRENTLY with T1.1. A guard body led by a Promela keyword SHALL be classified as a statement ALWAYS, never as a guard condition.

#### Scenario: T1.3 keyword-only guard body
GIVEN a guard body containing only `skip`
WHEN T1.3 runs
THEN the guard SHALL have no condition and SHALL have `skip` as its body statement.
