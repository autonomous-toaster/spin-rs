# Promela Parity
## Task Reference

| Task ID | Description |
|---------|-------------|
| T3.1 | else guard decided against the other guards of its selection |
| T3.2 | unless escape edge |
| T3.3 | timeout enabled only when no process can move |
| T3.4 | provided clause enforced |
| T3.5 | d_step restrictions enforced or refused |
| T3.6 | Deadlock defined as invalid end state |
| T4.1 | Integer type ranges and wraparound |
| T4.2 | Arrays including multi-dimensional |
| T4.3 | typedef and structured fields |
| T4.4 | Remote references and process identifiers |
| T4.5 | Channel surface |
| T5.1 | Until and Release in the formula type and translation |
| T5.4 | never claims |
| T5.5 | progress, accept and end labels, non-progress cycles |
| T6.1 | Audit of mechanisms that participate in nothing |
| T6.2 | Test failing when a state key is read by nothing |

## ADDED Requirements

### Requirement: Declared Constructs Are Honored
T6.1 SHALL complete BEFORE T3.4 SHALL run. A construct that the parser accepts SHALL be either implemented or refused ALWAYS with a diagnostic that names it.

#### Scenario: provided clause
GIVEN a proctype carrying a provided clause that is false
WHEN T3.4 has run
THEN the proctype SHALL not be executable, and the model SHALL not be reported clean on the strength of ignoring the clause.

### Requirement: Statement Semantics Match pan
T3.1 SHALL complete BEFORE T3.6 SHALL run. Statement semantics SHALL match ALWAYS pan's on every shape the generator produces.

#### Scenario: else guard
GIVEN a selection whose first guard is true
WHEN T3.1 has run and the model is compared with pan
THEN the else branch SHALL be unreachable in both engines.

#### Scenario: timeout
GIVEN a model in which one process is always enabled
WHEN T3.3 has run and the model is compared with pan
THEN timeout SHALL never be enabled, in both engines.

### Requirement: Deadlock Means Invalid End State
T3.6 SHALL complete BEFORE T5.6 SHALL run. A state with no enabled transition SHALL be reported ALWAYS as an error only when some process is not at a valid end state.

#### Scenario: completed model
GIVEN a model in which every process has run to its last position
WHEN T3.6 has run
THEN the run SHALL report no error, matching pan's treatment of a valid end state.

### Requirement: Property Surface
T5.1 SHALL complete BEFORE T5.4 SHALL run. Every temporal operator that pan accepts SHALL be decided ALWAYS or reported as a named decline.

#### Scenario: until
GIVEN a claim using the until operator
WHEN T5.1 has run
THEN the claim SHALL be decided, and the verdict SHALL match pan's.

#### Scenario: nested temporal
GIVEN the claim that eventually always b holds
WHEN T5.1 has run on a model where b is never set
THEN the verdict SHALL be a violation, matching pan's, rather than a decline.

### Requirement: Data Surface
T4.1 SHALL complete BEFORE T4.5 SHALL run. Type ranges, arrays, structures, remote references and the channel surface SHALL match ALWAYS pan's semantics.

#### Scenario: byte overflow
GIVEN an assignment of 256 to a byte
WHEN T4.1 has run and the model is compared with pan
THEN the stored value SHALL be the same in both engines.

### Requirement: Inert Mechanisms Are Defects
T6.1 SHALL complete BEFORE T6.2 SHALL run. A state key or AST field that is written or parsed and read by nothing SHALL fail the suite ALWAYS.

#### Scenario: unread state key
GIVEN a generated state key that no transition reads
WHEN T6.2 runs
THEN the suite SHALL fail and SHALL name the key.
