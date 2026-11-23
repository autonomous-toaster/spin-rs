# Promela Parity
## Task Reference

| Task ID | Description |
|---------|-------------|
| T3.1 | else guard decided against the other guards of the containing selection |
| T3.2 | unless escape edge, with the handler taking priority |
| T3.3 | timeout enabled only when no process can move |
| T3.4 | provided clause enforced |
| T3.5 | d_step restrictions enforced or refused |
| T3.7 | Fairness survey: a decision or a recorded absence of an oracle |
| T3.8 | run refused, with no oracle to encode against |
| T3.9 | priority enforced as a filter on the enabled set |
| T3.10 | A bare block is not a statement; unless is named |
| T3.12 | A statement separator and an increment parse |
| T4.2 | Arrays, including multi-dimensional |
| T4.3 | typedef and structured fields |
| T4.4 | Remote references and process identifiers |
| T4.5 | Channel surface, including rendezvous |
| T5.1 | Until and Release in the formula type and the translation |
| T5.2 | Nested temporal operators |
| T5.3 | Atom evaluation over the state |
| T5.4 | never claims |
| T5.5 | progress, accept and end labels, non-progress cycles |
| T5.6 | Liveness verdict parity on the generated corpus |
| T6.1 | Audit of mechanisms that participate in nothing |
| T6.2 | Test failing when a state key is read by nothing |

## ADDED Requirements

### Requirement: Declared Constructs Are Honored
The engine SHALL either implement ALWAYS a construct that the parser accepts, or refuse the construct with a diagnostic that names the construct (T3.4, T6.1, T6.2).

A mechanism that is parsed and referenced nowhere is worse than a refusal, because the model runs and the construct does nothing.

#### Scenario: provided clause
GIVEN a proctype carrying a provided clause that is false
WHEN T3.4 has run
THEN the proctype SHALL not be executable, and the model SHALL not be reported clean on the strength of ignoring the clause.

#### Scenario: a key that nothing reads
GIVEN a state key written at initialisation
WHEN T6.2 runs
THEN a key that no transition reads SHALL fail the test.

### Requirement: Statement Semantics Match pan
A statement SHALL produce ALWAYS the set of states that pan produces for the statement (T3.1, T3.12).

An else guard is decided against the other guards of the same selection, a statement separator is absorbed after every statement, and an increment statement assigns the incremented value (T3.1, T3.12).

#### Scenario: else guard
GIVEN a selection whose first guard is true
WHEN T3.1 has run and the model is compared with pan
THEN the else branch SHALL be unreachable in both engines.

#### Scenario: a separator between statements
GIVEN a body of two assignments separated by an arrow
WHEN T3.12 has run
THEN the model SHALL parse and both assignments SHALL run in order.

### Requirement: The Escape Has Priority
The escape statement SHALL take priority ALWAYS over the body statement that the escape guards (T3.2).

A handler that is merely enabled beside the body allows a run past the escape, and pan rejects that run.

#### Scenario: handler armed by the first statement
GIVEN `do :: x < 5 -> x = 1; x = x + 10 od unless (x == 1)`
WHEN T3.2 has run
THEN the loop SHALL be left before the second statement of the body runs.

#### Scenario: control with the escape checked only at the loop head
GIVEN the same body with the escape checked at the loop head instead
WHEN T3.2 has run
THEN the body SHALL complete and the claim SHALL be refuted, as pan refutes the claim.

### Requirement: Priority Filters The Enabled Set
Only the highest-priority enabled process SHALL move ALWAYS when a proctype declares a priority (T3.9).

The filter is applied after enabledness is known, so a high-priority process that is blocked does not hold up a low-priority process.

#### Scenario: a looping high-priority process
GIVEN a process that loops forever at priority 2 beside a process that sets a flag
WHEN T3.9 has run
THEN the flag SHALL never be set, and the claim about the flag SHALL hold.

#### Scenario: the same model without the clause
GIVEN the same model with no priority clause
WHEN T3.9 has run
THEN the flag SHALL be set, and the claim SHALL be refuted.

### Requirement: Rendezvous Is A Handshake
A send on a channel of capacity zero SHALL complete ALWAYS only when a receive takes the message (T4.5).

The send stages the message and the continuation without advancing, and the receive releases the stager; a buffered approximation would let the send complete before the handshake and would refute a claim that pan accepts.

#### Scenario: an ordering claim through a rendezvous
GIVEN `Sender: c ! 1; after = true` and `Receiver: c ? handshake` on a channel of capacity zero
WHEN T4.5 has run
THEN the claim `[] ( after -> handshake )` SHALL hold, as pan holds the claim.

#### Scenario: liveness over the handshake
GIVEN the same handshake and the claim `<> ( got )`
WHEN T4.5 has run
THEN the claim SHALL hold, and the opposite claim SHALL be refuted.

### Requirement: A Claim Is A Monitor
The checker SHALL report ALWAYS a never claim as a violation on completion (T5.4).

The verdict corresponds to `pan -a`, because pan needs the flag to look for acceptance cycles and the engine always looks. A failed assert inside a claim and a cycle through an accept-labelled state are violations for the same reason: each one is a run that pan reports.

#### Scenario: a claim that completes
GIVEN `never { skip }` over a model that terminates
WHEN T5.4 has run
THEN the run SHALL be reported as a violation.

#### Scenario: a claim whose guard never holds
GIVEN a claim whose only guard names a state that no reachable state satisfies
WHEN T5.4 has run
THEN no violation SHALL be reported, and an accept-labelled target of that branch SHALL NOT be treated as reachable.

### Requirement: An Atom Is Decided Or Declined By Name
The evaluator SHALL evaluate ALWAYS an atom against the state when the atom is an expression (T5.3).

An atom that names nothing the state carries is declined as an unsupported property, and an atom that is not an expression is declined as an unparsable formula, because the two are different debts.

#### Scenario: a comparison atom
GIVEN the claim `[] ( x <= 2 )` over a loop that increments `x`
WHEN T5.3 has run
THEN the claim SHALL be refuted, as pan refutes the claim.

#### Scenario: a fragment
GIVEN a formula whose atom is a fragment that no expression parser accepts
WHEN T5.3 has run
THEN the claim SHALL be reported as an unparsable formula, and the fragment SHALL be named.

#### Scenario: a biconditional
GIVEN the claim `[] ( a <-> b )`
WHEN T5.3 has run
THEN both implications SHALL be required, and the claim SHALL be decided rather than declined.

### Requirement: Temporal Operators Are Translated Or Declined
The translation SHALL decline ALWAYS a temporal operator that the translation does not support, with a diagnostic that names the operator (T5.1, T5.2).

Until and Release are part of the formula type; a nested operator such as `[]<>p` is declined rather than approximated, because an approximation reports the opposite verdict.

#### Scenario: nested temporal operator
GIVEN the claim `[] <> ( a )`
WHEN T5.2 has run
THEN the claim SHALL be reported as unchecked, and no verdict SHALL be produced.

### Requirement: Liveness Verdicts Agree With pan
The checker SHALL decide ALWAYS a liveness claim as pan decides the claim on the generated corpus (T5.5, T5.6).

#### Scenario: liveness over the corpus
GIVEN the generated corpus with the liveness claims
WHEN T5.6 has run
THEN every liveness claim SHALL agree with pan, or SHALL be counted as a decline.

### Requirement: Remaining Constructs Are Refused By Name
The engine SHALL refuse ALWAYS a construct that the engine does not implement, with a diagnostic that names the construct, or record the absence of an oracle (T3.3, T3.5, T3.7, T3.8, T3.10).

A refusal is a debt that a caller can act on; a plausible encoding is a wrong verdict that looks like a right verdict.

#### Scenario: run
GIVEN a body that spawns a process with run
WHEN T3.8 has run
THEN the run SHALL fail with a diagnostic naming run, and no verdict SHALL be produced.

#### Scenario: fairness
GIVEN a liveness claim and a request for fairness
WHEN T3.7 has run
THEN the request SHALL be refused, because no model exists that tells a fair from an unfair accepting cycle.

#### Scenario: a bare block
GIVEN a body that contains a bare block
WHEN T3.10 has run
THEN the model SHALL fail to parse, and the README SHALL say that the block is not a statement.

### Requirement: Data Surface Is Either In The State Or Refused
The state SHALL carry ALWAYS a data construct, or the engine SHALL refuse the construct with a diagnostic that names the construct (T4.2, T4.3, T4.4).

An array element is part of the state; a structured field and a remote reference are refused, because a field that is absent from the state makes every guard that reads the field wrong.

#### Scenario: an array element
GIVEN a model that writes `arr[1]` and reads `arr[1]`
WHEN T4.2 has run
THEN the value SHALL be part of the state and the claim SHALL be decided.

#### Scenario: a structured field
GIVEN a model that assigns a field of a typedef record
WHEN T4.3 runs
THEN the run SHALL fail with a diagnostic naming the field, and no verdict SHALL be produced.
