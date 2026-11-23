# Execution Model
## Task Reference

| Task ID | Description |
|---------|-------------|
| T2.1 | CFG builder: nodes are positions between statements, edges are statements |
| T2.2 | Emit one transition per edge, guarded on the source position, effect carrying the successor |
| T2.3 | Collapse the five counters into one program counter and derive termination from the counter |
| T2.4 | Fuse atomic and d_step regions into one edge |
| T2.5 | Compile goto, label and break to edge targets |
| T2.6 | Compile for loops to an init edge, a loop head and an exit edge |
| T2.7 | Sequencing sweep: statement count x kind x observed-write position against pan |
| T2.8 | Remove the shape-specific repairs that the CFG makes unnecessary |
| T2.10 | Nested do/od as a step in the shared step space |
| T2.11 | for loops as steps: init, loop head, exit |
| T2.12 | atomic and d_step fused into one step |
| T2.13 | A nested if whose branch is another if |
| T2.14 | Stop the step machine from swallowing a statement no encoding exists for |
| T2.15 | A statement parser absorbs the optional trailing terminator |
| T2.16 | A for loop followed by another statement parses |
| T3.6 | Deadlock defined as pan's invalid end state |
| T3.11 | Guards read with Promela truthiness |
| T4.1 | Integer types truncate on assignment |
| T4.6 | A channel queue lives in the state |

## ADDED Requirements

### Requirement: Positioned Enabling
The runtime SHALL enable ALWAYS a transition only at the position that the program counter of the owning proctype designates (T2.1, T2.2, T2.7).

Two statements of one body are never enabled at the same position, and the position is a single counter per proctype.

#### Scenario: two sequential statements
GIVEN a proctype body of two assignments
WHEN T2.7 runs the sequencing sweep
THEN the search SHALL NOT reach a state in which the second assignment has taken effect while the first has not.

#### Scenario: statement before a loop
GIVEN a proctype body of one assignment followed by a do loop
WHEN T2.7 runs the sequencing sweep
THEN the loop body SHALL NOT be enabled before the assignment has run.

### Requirement: One Counter Per Proctype
The generator SHALL retain AT MOST ONE program counter per proctype (T2.3, T2.9).

A construct that needs a sequencing counter of the construct's own would be a second source of position, and the generator retains no second counter: the counters the earlier emitters carried are deleted, not merged.

#### Scenario: counter inventory
GIVEN any generated model
WHEN T2.3 has run
THEN the initial state SHALL carry exactly one position counter per proctype, and the previous per-construct counters SHALL be absent.

### Requirement: Control Transfer Is An Edge
A control transfer SHALL write ALWAYS only the successor position of the edge (T2.5, T2.15).

No control transfer writes the termination flag: a transfer that did would end the process and leave the target unreachable, which is how a goto came to skip a label that existed.

#### Scenario: goto reaches its label
GIVEN a proctype body with a goto that skips an assignment and a label after it
WHEN T2.5 has run
THEN the skipped assignment SHALL be unreachable and the labelled statement SHALL be reachable.

#### Scenario: break leaves its loop
GIVEN a do loop whose body ends in a break followed by a trailing statement
WHEN T2.5 has run
THEN the trailing statement SHALL be reachable exactly after the break and the loop heads SHALL not be.

#### Scenario: a for loop followed by a statement
GIVEN a body of one for loop and one assignment after the loop
WHEN T2.16 has run
THEN the assignment SHALL be reachable once the range is exhausted.

### Requirement: A State Determines Its Own Transitions
The state SHALL carry ALWAYS every input that a transition guard reads (T4.6).

A channel queue is part of the state, because a queue is not static; a channel's capacity may be read from the declaration, because capacity is static. Expanding one state twice therefore yields the same set of transitions.

#### Scenario: reopening a state after a send
GIVEN a model with a buffered channel, a send and a claim on a later write
WHEN the search reopens the initial state after having taken the send
THEN the send SHALL be enabled exactly as it was the first time.

#### Scenario: the oracle disagrees about a terminating model
GIVEN a model that pan accepts and that our search calls an accepting cycle at trail length zero
WHEN the runtime is inspected
THEN no transition SHALL depend on any state the state table does not carry.

### Requirement: Guards Read As Promela Reads Them
The evaluator SHALL treat ALWAYS a guard as true exactly when the value is non-zero and neither false nor nil (T3.11, T4.1).

A stored value is truncated to the declared type of the variable on assignment, and division and remainder truncate toward zero as C does (T4.1). The host language's rules are not the model's rules: Lua treats zero as true, divides in floating point, floors the remainder, and Lua numbers have no width. Each of the three produced a verdict that pan disagrees with.

#### Scenario: a zero-valued guard
GIVEN `byte n = 0` and `if :: (n) -> a = 1 :: else -> skip fi`
WHEN T3.11 has run
THEN the first branch SHALL be disabled and the claim SHALL hold, as pan holds the claim.

#### Scenario: an assignment that overflows
GIVEN `byte x = 255` and `x = x + 1`
WHEN T4.1 has run
THEN `x` SHALL be 0.

#### Scenario: a negative remainder
GIVEN `short x = 0 - 7`
WHEN T4.1 has run
THEN `x % 2` SHALL be -1.

### Requirement: A Loop Blocks At Its Head
The engine SHALL block ALWAYS a do statement at the head when no guard of the do statement is enabled, as pan's invalid end state does (T2.11, T3.6).

A for statement leaves the range for the statement after the loop, and the body of a for statement is a sequence of ordinary steps rather than one atomic region (T2.11).

#### Scenario: loop whose guard stops holding
GIVEN `do :: (n < 3) -> n = n + 1 od` followed by a statement
WHEN T3.6 has run
THEN the checker SHALL report the process as blocked rather than reaching the statement after the loop.

#### Scenario: two writes inside a for
GIVEN `for (i : 0 .. 1) { d = 1; b = 1 }` and `[] ( d -> b )`
WHEN T2.11 has run
THEN the state in which `d` is set and `b` is not SHALL be reachable.

### Requirement: Termination Is Derived
The engine SHALL derive ALWAYS termination from the program counter of the owning proctype rather than from a flag written by statements (T2.3).

#### Scenario: completed process
GIVEN a proctype whose body has run to the last position
WHEN T2.3 has run
THEN the process SHALL report no enabled transition, and the derived condition SHALL be the only reason.

### Requirement: Folds Retire With Their Cause
The generator SHALL remove a shape-specific repair AFTER the control-flow graph covers the shape that the repair protected (T2.8, T2.10, T2.12, T2.13, T2.16).

The regression for a removed repair is held by the sequencing sweep, and a nested do, a fused region, a nested if and a trailing statement are covered by steps rather than by a special case.

#### Scenario: removal keeps the verdict
GIVEN a shape previously protected by a special case, such as a multi-statement branch in a nested if
WHEN T2.8 removes the special case
THEN T2.7 SHALL report the same verdict as pan for that shape.

### Requirement: An Unencodable Statement Is Refused
The generator SHALL refuse ALWAYS a statement that the step machine cannot express, rather than advancing past the statement (T2.14).

A statement dropped from the step space is a silent wrong: the model runs, the statement never executes, and a claim about the dropped effect comes back clean.

#### Scenario: a statement with no encoding
GIVEN a body containing a statement kind that the step machine does not encode
WHEN T2.14 has run
THEN the run SHALL fail with a diagnostic naming the statement kind, and no verdict SHALL be produced.

### Requirement: A Region Runs As One Step
The generator SHALL fuse ALWAYS an atomic or d_step region into one transition, conjoining the guards and concatenating the effects (T2.4).

A region that is not fused makes the states between the statements of the region observable, which pan does not do; a region containing control flow is refused rather than emitted as a region that is not atomic.

#### Scenario: two writes inside an atomic region
GIVEN `atomic { b = 1; a = 1 }` and `[] ( a -> b )`
WHEN T2.4 has run
THEN the state in which `a` is set and `b` is not SHALL be unreachable.

### Requirement: A for Loop Is A Loop
The generator SHALL lay out ALWAYS a for statement as an initialisation, a loop head and an exit (T2.6).

The body of a for statement is not an atomic region: pan makes the states between the statements of the body observable, and expanding the statement to an atomic region hid a refutation that pan finds.

#### Scenario: range exhausted
GIVEN `for (i : 0 .. 2) { c = 1 }` followed by an assignment
WHEN T2.6 has run
THEN the assignment SHALL be reachable after the third iteration.

