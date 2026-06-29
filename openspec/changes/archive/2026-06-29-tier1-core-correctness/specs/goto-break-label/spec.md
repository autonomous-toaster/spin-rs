# Goto / Break / Label
## Task Reference

| Task ID | Description |
|---------|-------------|
| T1.1 | Assign sequential step numbers to all labels in a proctype body during codegen |
| T1.2 | Emit `_step` variable in state vector for each proctype |
| T1.3 | Emit transitions for `goto label` |
| T1.4 | Emit transitions for `break` |
| T1.5 | Emit transitions for labels |
| T1.6 | Test: goto within a single proctype |
| T1.7 | Test: break exits do-loop correctly |
| T1.8 | Test: label as goto target is reachable |


## ADDED Requirements

### Requirement: Step Variable
T1.2 SHALL complete BEFORE T1.3 SHALL run. Each proctype SHALL have a `_step` variable in the state vector that tracks the current program counter.

#### Scenario: Step Variable scenario

GIVEN the requirement is implemented
WHEN the system runs
THEN the behavior SHALL be correct

### Requirement: Goto Transitions
T1.3 SHALL complete BEFORE T1.6 SHALL run. A `goto label` transition SHALL set `_step` to the target label's step number. The guard SHALL check that `_step` equals the current step number.

#### Scenario: GOTO-1: Goto within a proctype
GIVEN a proctype with `goto target` and `target:` label later in the body
WHEN T1.3 and T1.5 run
THEN the state exploration SHALL reach the label's body after executing the goto.

### Requirement: Break Transitions
T1.4 SHALL complete BEFORE T1.7 SHALL run. A `break` transition SHALL set `_step` to the exit step of the enclosing do-loop. The exit step SHALL be computed during codegen.

#### Scenario: BREAK-1: Break exits do-loop
GIVEN a do-loop with a break statement
WHEN T1.4 runs
THEN the break SHALL cause the process to exit the loop and continue after the od.

### Requirement: Label Transitions
T1.5 SHALL complete BEFORE T1.8 SHALL run. A label transition SHALL have a guard checking `_step == label_step` and an effect executing the label's body statements.

#### Scenario: Label Transitions scenario

GIVEN the requirement is implemented
WHEN the system runs
THEN the behavior SHALL be correct
