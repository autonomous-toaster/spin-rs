# verdict-completeness Specification

## Purpose
TBD - created by archiving change client-defects-s1-s4. Update Purpose after archive.

## Requirements

### Requirement: Finite Runs Refute Liveness
T1.1 SHALL complete BEFORE T1.2 SHALL run. The product search SHALL extend ALWAYS a terminal model state with a stuttering self-loop, so that a `<>` claim can be refuted over a finite run.

#### Scenario: T1.1 violated eventually in a terminating model
GIVEN a terminating model where p is never true
WHEN T1.1 checks `<>p`
THEN a violation SHALL be reported, matching spin.

### Requirement: Conditional Constraints Are Decided
T2.1 SHALL complete BEFORE T2.5 SHALL run. The checker SHALL decide ALWAYS the shape `[] (a -> <> b)` against the automaton for `<> (a && [] !b)`.

#### Scenario: T2.1 satisfied conditional
GIVEN a model where the antecedent never fires
WHEN T2.1 checks `[] (a -> <> b)`
THEN no error SHALL be reported and the property SHALL NOT be listed as unverified.

### Requirement: Undecided Is Not Invalid
T2.2 SHALL complete BEFORE T2.4 SHALL run. A property that produced no verdict SHALL be reported ALWAYS as unverified, never as a violation and never as success.

#### Scenario: T2.2 unsupported shape
GIVEN an LTL shape the automaton builder cannot complement
WHEN T2.2 checks it
THEN the result SHALL list it in `unverified` and SHALL report zero errors.

### Requirement: Truncation Is Visible
T2.3 SHALL complete BEFORE T2.4 SHALL run. A search that stops at `max_states` SHALL record ALWAYS that it did not close.

#### Scenario: T2.3 state limit
GIVEN a model larger than the configured state limit
WHEN T2.3 runs
THEN `unverified` SHALL contain an entry naming the limit.

### Requirement: No Success On An Incomplete Run
T2.4 SHALL complete BEFORE T3.1 SHALL run. The command line SHALL NOT print a success verdict ALWAYS when any property is unverified.

#### Scenario: T2.4 inconclusive run
GIVEN a run with a clean error count and one unverified property
WHEN T2.4 prints the result
THEN it SHALL report the run as inconclusive.

### Requirement: Documented Release Boundary
T3.1 SHALL complete BEFORE T3.3 SHALL run. The README SHALL document ALWAYS the three outcomes, the `Err` conditions, and the operators that are not supported.

#### Scenario: T3.1 unsupported operator
GIVEN a reader consulting the LTL feature list
WHEN T3.1 has run
THEN `U` and `V` SHALL NOT be listed as supported.
