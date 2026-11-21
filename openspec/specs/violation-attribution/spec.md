# violation-attribution Specification

## Purpose
TBD - created by archiving change s5-violation-attribution. Update Purpose after archive.

## Requirements

### Requirement: Violations Carry Their Claim Name
T1.1 SHALL complete BEFORE T1.3 SHALL run. A violation SHALL carry ALWAYS the name of the claim it refutes, including liveness claims.

#### Scenario: T1.1 refuted liveness claim
GIVEN a model with a named liveness claim that is violated
WHEN T1.1 reports the violation
THEN `property_name` SHALL equal the claim name.

### Requirement: Model Violations Are Not Claims
T1.2 SHALL complete BEFORE T1.3 SHALL run. A violation that is not a claim SHALL carry ALWAYS a name that no caller can mistake for one.

#### Scenario: T1.2 deadlock
GIVEN a model that deadlocks
WHEN T1.2 names the violation
THEN `property_name` SHALL be `deadlock`.

### Requirement: One Witness Per Accepting Run
T2.1 SHALL complete BEFORE T2.2 SHALL run. The complement automaton SHALL commit ALWAYS to a single witness point, so a run where the obligation keeps being discharged is never accepted as a violation.

#### Scenario: T2.1 satisfied conditional
GIVEN a model where the consequent arrives repeatedly
WHEN T2.1 checks `[] (a -> <> b)`
THEN no violation SHALL be reported.

### Requirement: Propositional Obligations
T2.2 SHALL complete BEFORE T2.3 SHALL run. The automaton guards SHALL express ALWAYS any propositional combination of atoms, so an unsatisfiable obligation is refuted rather than reported as unsupported.

#### Scenario: T2.2 unsatisfiable obligation
GIVEN the claim `[] (a -> <> (x && !x))` where the antecedent fires
WHEN T2.2 checks it
THEN a violation SHALL be reported.

### Requirement: Incomplete Results Are Always Shown
T2.3 SHALL complete AFTER T2.2. The command line SHALL report ALWAYS the properties it did not decide, whether or not it also reports violations.

#### Scenario: T2.3 partial run
GIVEN a run with one violation and one unverified property
WHEN T2.3 prints the result
THEN both SHALL appear.
