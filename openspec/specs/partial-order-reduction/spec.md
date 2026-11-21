# partial-order-reduction Specification

## Purpose
TBD - created by archiving change partial-order-reduction. Update Purpose after archive.

## Requirements

### Requirement: Declared Dependencies
T1.1 SHALL complete BEFORE T1.3 SHALL run. Every generated transition SHALL declare ALWAYS the variables its guard reads and its effect writes, taken from the AST rather than parsed out of a label.

#### Scenario: T1.1 metadata present
GIVEN any generated model in the corpus
WHEN T1.1 has run
THEN each transition SHALL carry `reads`, `writes` and `internal`, and the sets SHALL match the effect closure.

### Requirement: No Dormant Reduction
T1.2 SHALL complete BEFORE T1.4 SHALL run. A flag that claims to enable reduction SHALL either enable a working reduction or fail ALWAYS with a diagnostic naming it as unimplemented.

#### Scenario: T1.2 flag honesty
GIVEN `--por` on a model with independent processes
WHEN T1.2 has run
THEN the run SHALL either reduce the state space or report that reduction is unavailable, and SHALL NOT silently explore the unreduced space.

### Requirement: Reduction Preserves Verdicts
T2.1 SHALL complete BEFORE T2.3 SHALL run. The reduced search SHALL produce ALWAYS the same violations and unverified entries as the unreduced search, compared by claim name and description class.

#### Scenario: T2.1 independent processes
GIVEN a model of five independent proctypes each with three statements
WHEN T2.1 runs with reduction enabled
THEN the verdicts SHALL match the unreduced search and the state count SHALL be lower.

### Requirement: Liveness Provsio
T2.2 SHALL complete BEFORE T2.3 SHALL run. An accepting cycle SHALL NOT be hidden ALWAYS by reduction, so a cycle composed only of internal transitions SHALL still be explored.

#### Scenario: T2.2 liveness preserved
GIVEN a model with a violated liveness claim whose accepting cycle consists of internal transitions
WHEN T2.2 runs with reduction enabled
THEN the violation SHALL still be reported.

### Requirement: Budget Bounds The Run
T2.4 SHALL complete BEFORE T3.2 SHALL run. `max_states` SHALL bound ALWAYS the whole run across per-claim traversals, not each traversal, and a claim that cannot be decided within the remaining budget SHALL be reported in `unverified` with kind `StateBudget` naming that claim.

#### Scenario: T2.4 budget exhausted mid-run
GIVEN a run of five claims whose budget is exhausted during the third traversal
WHEN T2.4 reports the result
THEN the third, fourth and fifth claims SHALL each appear in `unverified` with kind `StateBudget`, and SHALL NOT be absent.

#### Scenario: T2.4 declaration order decides who is starved
GIVEN a model whose first claim is expensive and whose later claims are cheap
WHEN T2.4 runs with a budget the first claim exhausts
THEN the later claims SHALL be reported as `StateBudget`, and the documentation SHALL state that claim declaration order determines which claims are starved.

### Requirement: Dependent Guards Are Not Reduced Away
T2.5 SHALL complete BEFORE T3.1 SHALL run. An ample set SHALL NOT exclude ALWAYS a transition whose guard reads a variable another enabled transition writes.

#### Scenario: T2.5 chain of dependent guards
GIVEN a chain of proctypes where each guard reads its predecessor's completion flag
WHEN T2.5 runs with reduction enabled
THEN the verdicts SHALL match the unreduced search, so no violation behind such a guard is missed.

### Requirement: Gated Parity
T3.2 SHALL complete BEFORE T4.1 SHALL run. Reduction SHALL be enabled by default ONLY IF the differential harness passes on the whole corpus, comparing against both pan and the unreduced search.

#### Scenario: T3.2 parity metric
GIVEN the corpus of chain, concurrent-phase and independent-process fixtures
WHEN T3.2 runs
THEN our states for a complete all-property verdict SHALL be no greater than the sum of pan's per-claim traversals, and error counts SHALL match per claim.
