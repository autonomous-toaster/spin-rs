# Per State Cost

## ADDED Requirements

### Requirement: A Rate Is Reported Only On A Fixture That Can Carry It

T1.1 SHALL complete BEFORE T4.1 SHALL run.

The engine starts in 3-4 ms, and the N=8 chain explores 65 states. A rate measured where startup dominates measures startup, so the rate comes from the committed N=60 fixture instead.

T1.2 SHALL complete BEFORE T1.3 SHALL run.

T1.4 SHALL ALWAYS report the fixture size and the startup share beside the rate.

#### Scenario: T1.4 a rate carries its fixture

GIVEN the committed throughput fixture
WHEN T1.4 reports states per second
THEN the report SHALL name the fixture size and the startup share, and SHALL NOT present the N=8 chain as a throughput figure.

### Requirement: Cost Is Reported Per Entry And Per Transition

T1.3 SHALL complete BEFORE T1.5 SHALL run.

Per-state cost has two independent drivers: a wide state and a busy one. The record inherited from `veriplan-perf-parity` reported only states per second and conflated them.

T1.5 SHALL record ALWAYS microseconds per entry and per transition, at both transition densities.

#### Scenario: T1.5 the baseline carries both densities

GIVEN the throughput fixture and a chain with five transitions per state
WHEN T1.5 records the baseline
THEN it SHALL report microseconds per entry and per transition for both, beside states per second.

### Requirement: No Whole-State Copy Or Reserialization Per Enabled Transition

T3.1 SHALL complete BEFORE T3.5 SHALL run.
T3.2 SHALL complete BEFORE T3.5 SHALL run.
T3.4 SHALL complete BEFORE T3.5 SHALL run.

Today `_spin_clone` copies every entry for each enabled transition, and `serialize_table` walks every successor into a fresh owned string that is then hashed and stored.

The engine SHALL ALWAYS copy only the entries an effect writes, and MUST NOT clone the whole state table for a transition.

#### Scenario: T3.2 write-set copy

GIVEN a state of 422 entries and an effect that writes one field
WHEN T3.2 expands a transition
THEN the successor SHALL carry one copied entry, not 422.

### Requirement: Storage Stays Exact

T3.3 SHALL keep ALWAYS full-state comparison in the store, and MUST NOT use hash-only storage.

`StorageMode::HashCompact` trades completeness for speed and cannot underwrite a count or a verdict.

#### Scenario: T3.3 exact store under the new representation

GIVEN any corpus model
WHEN T3.3 runs with the new state representation
THEN the store SHALL compare full states.

### Requirement: The Transition Table Is Not Rebuilt Per State

T2.1 SHALL complete BEFORE T2.4 SHALL run.
T2.3 SHALL complete BEFORE T2.4 SHALL run.

`_spin_get_transitions` calls every proctype function on every state visit, and each emitted step allocates a guard closure, an effect closure and a descriptor table. On a chain of N, N-1 processes are blocked and pay that allocation anyway.

T2.2 SHALL ALWAYS bind the state to the effect as an argument, and MUST NOT allocate a closure for a state visit.

#### Scenario: T2.3 blocked processes are not paid for

GIVEN a chain of 30 tasks where one is enabled
WHEN T2.3 expands a state
THEN no guard or effect closure SHALL be allocated for the 29 blocked tasks.

### Requirement: Per-Entry Cost Does Not Track Transition Density

T4.2 SHALL ALWAYS assert equal per-entry cost at two transition densities.

A wide state with one enabled transition and a wide state with five must cost the same per entry once the representation stops cloning per transition.

#### Scenario: T4.2 two densities, equal entries

GIVEN two committed fixtures of equal entry count, at about 1.2 and 5 enabled transitions per state
WHEN T4.2 measures them
THEN the cost per entry SHALL NOT rise with transition density.

### Requirement: The Floor Is Restated Where It Can Be Measured

T1.5 SHALL complete BEFORE T4.1 SHALL run.

Measured at v0.5.42: 1432 states per second at 422 entries. The target is 4x; reaching pan is a compiled transition relation, which this change does not attempt.

The engine SHALL ALWAYS sustain at least 5700 states per second on the committed throughput fixture in a release build, as the median of three runs.

T4.3 SHALL ALWAYS report the achieved figure with its fixture size and transition density, and MUST NOT call it parity with pan.

#### Scenario: T4.1 the floor holds

GIVEN the committed throughput fixture in a release build
WHEN T4.1 measures three runs
THEN the median SHALL be at least 5700 states per second.

### Requirement: No Verdict And No Count Changes

T5.1 SHALL complete BEFORE T5.2 SHALL run.

The work sits on the path every transition takes, and a cheaper transition that fires when it should not is not a speedup.

No violation, unverified entry or state count SHALL ALWAYS change for any model in the corpus.

T5.2 SHALL assert ALWAYS the chain probe counts at 49, 65, 81 and 97.

#### Scenario: T5.2 the chain counts are unchanged

GIVEN the chain probes at N=6, 8, 10 and 12
WHEN T5.2 runs both directions
THEN the counts SHALL be 49, 65, 81 and 97, and every verdict SHALL match pan.

### Requirement: Two Levers Do Not Run At Once

T2.1 and T3.1 SHALL NOT run concurrently.

Both rewrite the same expansion path, so a measurement taken with one half applied cannot be attributed.

#### Scenario: one lever at a time

GIVEN both levers unstarted
WHEN the work is measured
THEN T2.1 and T3.1 SHALL NOT be applied in the same measurement run.
