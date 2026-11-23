# Differential Conformance
## Task Reference

| Task ID | Description |
|---------|-------------|
| T1.1 | Structural model generator over a body-shape grammar |
| T1.2 | Pan oracle runner classifying agree, false refutation, false clean, decline, refusal, pan-unbuildable |
| T1.3 | Baseline report recorded against the current binary |
| T1.4 | Mutation test: the generator reproduces every known defect shape |
| T1.5 | CI gate failing on a wrong verdict, printing declines without subtracting declines from the total |
| T1.6 | A second corpus the author of pan wrote: the models spin itself ships |
| T2.9 | Delete the old emitters after the new path is green |
| T5.7 | A terminating model is not reported as an accepting cycle |

## ADDED Requirements

### Requirement: Structural Corpus
The differential corpus SHALL cover ALWAYS the body shapes that the grammar can produce (T1.1).

The corpus is not limited to the shapes a consumer is known to emit, and a shape found by hand is added to the generator before the fix that addresses the shape is called complete (T1.4).

#### Scenario: two-phase body
GIVEN a proctype body of two sequential do loops, the phase-one-then-phase-two shape
WHEN T1.1 generates the corpus
THEN that shape SHALL be present, and T1.5 SHALL report the verdict against pan.

#### Scenario: statement before a loop
GIVEN a proctype body of one assignment followed by a do loop
WHEN T1.1 generates the corpus
THEN that shape SHALL be present, and T1.5 SHALL report the verdict against pan.

### Requirement: A Second Corpus The Author Of Pan Wrote
The gate SHALL run ALWAYS a directory of models that the project did not write, given a directory (T1.6).

A corpus the same hand curates measures the curator: the generated corpus found thirty wrong verdicts where the hand-written corpus found four, and the models shipped with spin found six defects that the generated corpus could not reach.

#### Scenario: the shipped models
GIVEN the models under spin's Examples directory
WHEN T1.6 runs against pan
THEN each model SHALL be classified, and a model with more than one claim SHALL be named as not comparable rather than guessed at.

### Requirement: Verdict Gate
The gate SHALL fail ALWAYS on a refutation of a model that pan accepts (T1.2, T1.5).

A refutation of a valid model is the failure this gate exists to catch.

#### Scenario: false refutation
GIVEN a model that pan proves valid and that this engine reports with one error
WHEN T1.5 runs the gate
THEN the build SHALL fail and the report SHALL name the model.

#### Scenario: no oracle
GIVEN a machine without spin and gcc
WHEN T1.5 runs the gate
THEN the gate SHALL exit with a distinct status rather than reporting success, because an unmeasured corpus is not a passing corpus.

### Requirement: Verdict Gate In Reverse
The gate SHALL fail ALWAYS on a clean verdict for a model that pan refutes (T1.2, T1.5).

A clean verdict on an invalid model is the second failure this gate exists to catch.

#### Scenario: false clean
GIVEN a model that pan refutes and that this engine reports with no error
WHEN T1.5 runs the gate
THEN the build SHALL fail and the report SHALL name the model.

### Requirement: Declines Are Counted
The gate SHALL count ALWAYS a model that the engine does not decide as a decline, never as an agreement (T1.2).

A refusal and a decline are different debts: a decline is a traversal that ran out of budget or a construct that the engine does not implement, and a refusal is a model that never ran.

#### Scenario: declined model
GIVEN a model carrying a property that this engine declines to decide
WHEN T1.2 classifies the run
THEN the outcome SHALL be decline, and the total SHALL include the model rather than omit the model.

#### Scenario: agreement inflation
GIVEN a previously declined model whose error count happens to be zero
WHEN T1.2 classifies the run
THEN the outcome SHALL be decline and not agreement.

### Requirement: Baseline Before Rewrite
The baseline report SHALL record ALWAYS the outcome counts at the commit before the execution model changes (T1.3).

The baseline exists so that the effect of the execution model is measured rather than claimed.

#### Scenario: baseline recorded
GIVEN the binary at the commit before the execution model changes
WHEN T1.3 has run
THEN the report SHALL state, per outcome, how many models fall in each class and SHALL name every wrong verdict.

### Requirement: Generator Self-Test
The generator SHALL reproduce ALWAYS every shape known to be wrong (T1.4).

A grammar change that drops a known shape fails the gate, and the check is confirmed by reintroducing a fixed defect and observing the gate fail.

#### Scenario: generator regression
GIVEN the shapes known at the baseline, being a two-statement body, an assignment before a loop, two sequential loops, and an assert in a blocked process
WHEN T1.4 runs
THEN each shape SHALL be generated, and a grammar change that drops one SHALL fail the gate.

### Requirement: Nothing Leaves The Comparison Silently
The report SHALL name ALWAYS a model whose oracle could not be built and a model that the engine declines (T1.2).

A corpus that loses models to an unbuildable oracle looks better than the corpus is.

#### Scenario: oracle fails to build
GIVEN a model that pan refuses, for instance because two declarations share a name
WHEN the gate runs
THEN the model SHALL be listed as not compared, and the total SHALL include the model.

### Requirement: A Terminating Model Is Not An Accepting Cycle
The checker SHALL NOT report ALWAYS a model in which every process has completed as an accepting cycle (T5.7).

#### Scenario: finished process and a claim
GIVEN a model whose only process runs to completion and a claim that holds in the final state
WHEN T5.7 has run
THEN the verdict SHALL match pan's, and no accepting cycle SHALL be reported on the strength of a finished process.

### Requirement: The Old Path Retires Last
The generator SHALL delete ALWAYS the old statement emitters only once the gate is green on the new path (T2.9).

A green gate before the deletion makes a deletion that changes a verdict attributable.

#### Scenario: deletion keeps the gate green
GIVEN the corpus green on the flattened path
WHEN T2.9 deletes the old emitters
THEN T1.5 SHALL still report no wrong verdict.
