# State Projection

## ADDED Requirements

### Requirement: Projection Is Claim-Relative
T2.1 SHALL complete BEFORE T2.3 SHALL run. The projected state SHALL ALWAYS be computed per claim, from the field set that claim can observe, and MUST NOT be a single global dead-variable pass shared by all claims.

A global pass is not equivalent and buys less: the consumer's generator emits claims whose observable sets differ, so a global projection would retain the union and recover only part of the reduction.

#### Scenario: T2.7 two claims, two counts
GIVEN one model with a failure-branch bit that claim A reads and claim B does not
WHEN T2.7 runs both claims
THEN the state counts for A and B SHALL differ, and the union of their observable sets SHALL NOT be used for either.

### Requirement: Projection Preserves Findings
T2.2 SHALL complete BEFORE T2.3 SHALL run. The projection SHALL retain ALWAYS every field that can affect a finding: the assertion failure marker, and every field read by deadlock detection. A hidden bit MUST NOT be able to suppress an assertion failure or a deadlock.

Under a claim, `check_violation` short-circuits deadlock detection and reads only the assertion marker; with no claim it also reads the running-process and `_nr_pr` fields. Both sets are mandatory.

#### Scenario: T2.8 hidden bit cannot hide an assert
GIVEN a model whose only claim does not read the variable an `assert` failure is recorded in
WHEN T2.8 projects the state and explores
THEN the assertion failure SHALL still be reported.

#### Scenario: T2.9 hidden bit cannot hide a deadlock
GIVEN a claim-free model whose deadlock is only detectable from fields the previous projection dropped
WHEN T2.9 explores
THEN the deadlock SHALL still be reported.

### Requirement: Projection Preserves Verdicts
T2.10 SHALL complete BEFORE T2.11 SHALL run. No violation, unverified entry or clean verdict SHALL change ALWAYS for any model in the corpus, compared against pan per claim.

#### Scenario: T2.10 corpus sweep
GIVEN the full differential corpus
WHEN T2.10 runs with projection enabled
THEN every verdict SHALL match the pre-projection run and pan, and no claim SHALL move between `violations`, `unverified` and implicit-satisfied.

### Requirement: State Count Grows Linearly On The Consumer Shape
T1.4 SHALL complete BEFORE T2.11 SHALL run. On the consumer's chain shape - N active proctypes, one bit triplet per task, one failure branch per task, one claim that does not read the failure bits - the state count SHALL grow ALWAYS linearly in N, within a small constant factor of pan's.

Measured after the change, against pan's 37 / 49 / 61 at N=6 / 8 / 10. The claim must not refute, otherwise the space does not close and the count measures refutation depth rather than the projection.

#### Scenario: T2.11 linear growth
GIVEN the non-refuting chain fixture at N=6, 8 and 10
WHEN T2.11 measures states per claim
THEN the count SHALL be within a small constant factor of pan's, and the ratio between consecutive N SHALL NOT grow geometrically.

### Requirement: Early Refutation Is Pinned Separately
T1.5 SHALL complete BEFORE T1.6 SHALL run. The refuting fixture SHALL ALWAYS record pan's refutation depth and our own, because agreement on the verdict alone hides the cost gap.

pan refutes the consumer's `[] ( active_X -> done_Y )` claim in 3 states where we explore 11254. A verdict-only gate passes this and reports parity.

#### Scenario: T1.5 refutation depth recorded
GIVEN the refuting chain fixture
WHEN T1.5 runs
THEN pan's stored-state count at refutation SHALL be recorded and our count SHALL be asserted against it as a separate, named check.

### Requirement: An Identical Count Is Not Parity
T1.7 SHALL complete BEFORE T1.6 SHALL run. The gate SHALL NOT treat a count that matches pan on a refuting fixture as evidence of parity, because this search does not stop early.

Measured: 694 / 2806 / 11254 on both probe directions at N=6 / 8 / 10, identical whether the claim refutes or closes. A matching count therefore says only that we explored the whole space, and a swapped fixture would be invisible in our own output.

#### Scenario: T1.7 an identical count is not parity
GIVEN the refuting fixture
WHEN T1.7 compares our count with pan's
THEN the gate SHALL NOT accept a match as parity, because our count is identical in both probe directions and a swapped fixture is invisible in our output.

#### Scenario: T1.6 the two probe directions are not interchangeable
GIVEN the same chain model with the claim written in the refuting direction and in the closing direction
WHEN T1.6 runs both
THEN the fixture SHALL pin which direction establishes the state-count property, and a test SHALL fail if the two fixtures are swapped, because our count is identical in both directions and a swap would be invisible in our output.
