## Task Reference

| Task ID | Description |
|---------|-------------|
| T5.1 | Parse LTL formulas and never claims |
| T5.2 | Translate LTL to Büchi automaton |
| T5.3 | Run synchronous product for model checking |
| T5.4 | Implement nested DFS for liveness properties |

## ADDED Requirements

### Requirement: Parse LTL formulas

T5.1 SHALL parse LTL formulas using standard Spin syntax (`[]`, `<>`, `!`, `&&`, `||`, `->`, `<->`, `U`, `V`, `X`). T5.1 SHALL also parse Spin never claims. T5.1 SHALL complete BEFORE T5.2 SHALL translate to Büchi.

#### Scenario: Simple LTL formula

- **WHEN** T5.1 receives `[]!(x == 0)`
- **THEN** T5.1 SHALL parse it as globally-not (x equals 0)

#### Scenario: Never claim

- **WHEN** T5.1 receives a Spin `never { ... }` block
- **THEN** T5.1 SHALL parse it as an automaton description with states, transitions, and acceptance conditions

### Requirement: LTL to Büchi translation

T5.2 SHALL translate LTL formulas to non-deterministic Büchi automata using the ω-automata crate. T5.2 SHALL produce automata suitable for synchronous product with the model. T5.2 SHALL complete AFTER T5.1 SHALL parse the formula.

#### Scenario: Safety property

- **WHEN** T5.2 receives `[]!(x == 0)`
- **THEN** T5.2 SHALL produce a Büchi automaton whose language is all behaviors where x is never 0

#### Scenario: Liveness property

- **WHEN** T5.2 receives `<>(x == 1)`
- **THEN** T5.2 SHALL produce a Büchi automaton whose language is all behaviors where eventually x becomes 1

### Requirement: Synchronous product

T5.3 SHALL compute the synchronous product of the model's state space and the Büchi automaton (or never claim). T5.3 SHALL explore the combined state space where each step transitions both systems. T5.3 SHALL complete BEFORE T5.4 SHALL run nested DFS.

#### Scenario: Safety verification

- **WHEN** T5.3 explores a model where `x` is never 0 paired with a never claim for `[]!(x == 0)`
- **THEN** T5.3 SHALL explore all states, and if `x == 0` is reachable, SHALL report a violation

### Requirement: Nested DFS for liveness

T5.4 SHALL implement Spin's nested depth-first search algorithm for liveness property checking. The outer DFS finds accepting states; the inner DFS searches for cycles reachable from each accepting state. T5.4 SHALL complete AFTER T5.3 SHALL compute the product.

#### Scenario: Liveness violation

- **WHEN** T5.4 checks a model where `x` can stay non-zero indefinitely paired with `<>(x == 1)`
- **THEN** T5.4 SHALL detect if there is an infinite path where `x != 1` forever (acceptance cycle)

#### Scenario: Liveness satisfied

- **WHEN** T5.4 checks a model where `x` always eventually becomes 1
- **THEN** T5.4 SHALL report no acceptance cycles
