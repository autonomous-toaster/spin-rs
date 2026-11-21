# nested-guard-blocks Specification

## Purpose
TBD - created by archiving change s6-nested-if-codegen. Update Purpose after archive.

## Requirements

### Requirement: Nested If Branches Are Reachable
T1.1 SHALL complete BEFORE T1.3 SHALL run. An `if/fi` nested in a guard body SHALL contribute ALWAYS one transition per branch, so every branch's effect is reachable.

#### Scenario: T1.1 nested if assignment
GIVEN a proctype whose only assignment to `f` is inside an `if` nested in a `do`
WHEN T1.1 generates the guard transitions
THEN the branch setting `f` SHALL be enabled and the property `[] ( !(f) )` SHALL be refuted.

### Requirement: Unsupported Nested Blocks Fail Loudly
T1.2 SHALL complete BEFORE T1.3 SHALL run. A nested block the step machine cannot express SHALL produce ALWAYS an evaluation error, never a silently missing transition.

#### Scenario: T1.2 nested do
GIVEN a `do/od` nested inside a guard body
WHEN T1.2 emits it
THEN evaluating the run SHALL report that the model could not be checked.
