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
