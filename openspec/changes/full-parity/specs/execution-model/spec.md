# Execution Model
## Task Reference

| Task ID | Description |
|---------|-------------|
| T2.1 | CFG builder: nodes are positions between statements, edges are statements |
| T2.2 | Emit one transition per edge, guarded on the source position, effect carrying the successor |
| T2.3 | Collapse the five counters into one program counter and derive termination from it |
| T2.4 | Fuse atomic and d_step regions into one edge |
| T2.5 | Compile goto, label and break to edge targets |
| T2.6 | Compile for loops to an init edge, a loop head and an exit edge |
| T2.7 | Sequencing sweep: statement count x kind x observed-write position against pan |
| T2.8 | Remove the shape-specific repairs that the CFG makes unnecessary |
| T2.9 | Delete the old emitters after the new path is green |
| T4.6 | A state determines its own transitions: a channel queue lives in the state, not in the runtime |

## ADDED Requirements

### Requirement: Positioned Enabling
T2.2 SHALL complete BEFORE T2.7 SHALL run. Every generated transition SHALL be enabled ALWAYS only at the position that the program counter of its proctype designates, so that two statements of one body are never enabled at the same position.

#### Scenario: two sequential statements
GIVEN a proctype body of two assignments
WHEN T2.7 runs the sequencing sweep
THEN the search SHALL NOT reach a state in which the second assignment has taken effect while the first has not.

#### Scenario: statement before a loop
GIVEN a proctype body of one assignment followed by a do loop
WHEN T2.7 runs the sequencing sweep
THEN the loop body SHALL NOT be enabled before the assignment has run.

### Requirement: One Counter Per Proctype
T2.3 SHALL complete BEFORE T2.8 SHALL run. A proctype SHALL retain AT MOST ONE program counter, so that no construct introduces a sequencing counter of its own.

#### Scenario: counter inventory
GIVEN any generated model
WHEN T2.3 has run
THEN the initial state SHALL carry exactly one position counter per proctype, and the previous per-construct counters SHALL be absent.

### Requirement: Control Transfer Is An Edge
T2.5 SHALL complete BEFORE T2.6 SHALL run. A control transfer SHALL write ALWAYS only the successor position of its edge. No control transfer SHALL write ALWAYS the termination flag.

#### Scenario: goto reaches its label
GIVEN a proctype body with a goto that skips an assignment and a label after it
WHEN T2.5 has run
THEN the skipped assignment SHALL be unreachable and the labelled statement SHALL be reachable.

#### Scenario: break leaves its loop
GIVEN a do loop whose body ends in a break followed by a trailing statement
WHEN T2.5 has run
THEN the trailing statement SHALL be reachable exactly after the break and the loop heads SHALL not be.

### Requirement: A State Determines Its Own Transitions
T4.6 SHALL hold BEFORE any verdict SHALL be trusted. Every input a transition's guard reads SHALL be part of the state that the transition is expanded from, so that expanding the same state twice SHALL yield the same set of transitions.

A channel's queue SHALL be part of the state. Only its capacity MAY be read from the declaration, because capacity is static and a queue is not.

#### Scenario: reopening a state after a send
GIVEN a model with a buffered channel, a send and a claim on a later write
WHEN the search reopens the initial state after having taken the send
THEN the send SHALL be enabled exactly as it was the first time.

#### Scenario: the oracle disagrees about a terminating model
GIVEN a model that pan accepts and that our search calls an accepting cycle at trail length zero
WHEN the runtime is inspected
THEN no transition SHALL depend on any state the state table does not carry.

### Requirement: A Loop Blocks At Its Head
A `do` whose guards are all disabled at its head SHALL block, as pan's invalid end state does, and SHALL NOT fall through to the statement after it. Only a `break` leaves a `do`. A `for` SHALL leave its range for the statement after it, and its body SHALL be a sequence of ordinary steps rather than one atomic region.

#### Scenario: loop whose guard stops holding
GIVEN `do :: (n < 3) -> n = n + 1 od` followed by a statement
WHEN the guard becomes false
THEN the process SHALL be reported as blocked, matching pan's invalid end state, rather than reaching the statement after the loop.

#### Scenario: two writes inside a for
GIVEN `for (i : 0 .. 1) { d = 1; b = 1 }` and `[] ( d -> b )`
WHEN the claim is checked
THEN the state in which `d` is set and `b` is not SHALL be reachable, and the claim SHALL be refuted as pan refutes it.

### Requirement: Termination Is Derived
T2.3 SHALL complete BEFORE T2.9 SHALL run. Termination SHALL be derived ALWAYS from the program counter of its proctype rather than from a flag written by statements.

#### Scenario: completed process
GIVEN a proctype whose body has run to its last position
WHEN T2.3 has run
THEN the process SHALL report no enabled transition, and the derived condition SHALL be the only reason.

### Requirement: Folds Retire With Their Cause
T2.7 SHALL complete BEFORE T2.8 SHALL run. A shape-specific repair SHALL be removed AFTER the CFG covers the shape it repaired, and its regression SHALL be held by T2.7.

#### Scenario: removal keeps the verdict
GIVEN a shape previously protected by a special case, such as a multi-statement branch in a nested if
WHEN T2.8 removes the special case
THEN T2.7 SHALL still report the same verdict as pan for that shape.
