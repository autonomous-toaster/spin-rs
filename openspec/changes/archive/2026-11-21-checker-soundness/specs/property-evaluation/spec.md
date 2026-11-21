# Property Evaluation
## Task Reference

| Task ID | Description |
|---------|-------------|
| T3.1 | Unresolved atom is an error naming the atom |
| T3.2 | Reject non-identifier atoms explicitly |
| T3.3 | Evaluate atoms in the successor state |
| T4.2 | Compare per-property error counts |

## ADDED Requirements

### Requirement: Unresolved Atoms Are Never False
T3.1 SHALL complete BEFORE T3.2 SHALL run. An atomic proposition that does not resolve against the model state SHALL produce a diagnostic naming that atom ALWAYS, and SHALL NOT be evaluated as false.

#### Scenario: T3.1 unknown atom
GIVEN a property referring to an atom absent from every reachable state
WHEN T3.1 runs the product check
THEN the check SHALL abort with a diagnostic naming the atom.

### Requirement: Atom Form Is Explicit
T3.2 SHALL complete BEFORE T3.3 SHALL run. An atomic proposition that is not a bare identifier SHALL either be evaluated as an expression ALWAYS or be rejected as unsupported; it SHALL NOT resolve silently to false.

#### Scenario: T3.2 comparison atom
GIVEN the property atom `x == 0`
WHEN T3.2 runs
THEN the atom SHALL be evaluated against the state, or rejected as unsupported.

### Requirement: Successor State Evaluation
T3.3 SHALL complete BEFORE T4.2 SHALL run. Atomic propositions SHALL be evaluated in the successor state ALWAYS before a Büchi transition is synchronised with it, so that `[]` does not degenerate to `X`.

#### Scenario: T3.3 always versus next
GIVEN a model where a variable changes in the second step
WHEN T3.3 runs a `[]` property over it
THEN the verdict SHALL match real spin.
