# Strong Fairness

## Task Reference

| Task ID | Description |
|---------|-------------|
| T6.1 | Add per-transition counters to state vector |
| T6.2 | Increment enabled_count for each transition |
| T6.3 | Increment taken_count when transition is taken |
| T6.4 | Implement strong fairness check during cycle detection |
| T6.5 | Add --strong-fairness CLI option |
| T6.6 | Test: strong fairness detects violation |

## ADDED Requirements

### Requirement: Fairness Counters

T6.1 SHALL complete BEFORE T6.2 SHALL run. Each transition SHALL have `enabled_count` and `taken_count` in the state vector. T6.2 SHALL increment `enabled_count` for each transition in each state. T6.3 SHALL increment `taken_count` when a transition is taken.

#### Scenario: FAIR-1: Strong fairness violation

GIVEN a model where a transition is enabled infinitely often but never taken
WHEN T6.6 runs with `--strong-fairness`
THEN a strong fairness violation SHALL be reported.

### Requirement: Strong Fairness Check

T6.4 SHALL complete AFTER T6.3. During cycle detection, a transition is "fairly enabled" when its `enabled_count` grows unbounded along the cycle. A transition is "fairly taken" when `taken_count >= enabled_count` in the limit. When a transition is fairly enabled but not fairly taken, a strong fairness violation is reported.

#### Scenario: FAIR-1: Strong fairness violation

GIVEN a model where a transition is enabled infinitely often but never taken
WHEN T6.6 runs with `--strong-fairness`
THEN a strong fairness violation SHALL be reported.
