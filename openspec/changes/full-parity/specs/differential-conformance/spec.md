# Differential Conformance
## Task Reference

| Task ID | Description |
|---------|-------------|
| T1.1 | Structural model generator over a body-shape grammar |
| T1.2 | Pan oracle runner classifying agree, false refutation, false clean, decline, refusal, pan-unbuildable |
| T1.3 | Baseline report recorded against the current binary |
| T1.4 | Mutation test: the generator reproduces every known defect shape |
| T1.5 | CI gate failing on a wrong verdict, printing declines without subtracting them |

## ADDED Requirements

### Requirement: Structural Corpus
T1.1 SHALL complete BEFORE T1.3 SHALL run. The differential corpus SHALL cover ALWAYS the body shapes the grammar can produce, and MUST NOT be limited to the shapes a consumer is known to emit.

#### Scenario: two-phase body
GIVEN a proctype body of two sequential do loops, the phase-one-then-phase-two shape
WHEN T1.1 generates the corpus
THEN that shape SHALL be present, and T1.5 SHALL report its verdict against pan.

#### Scenario: statement before a loop
GIVEN a proctype body of one assignment followed by a do loop
WHEN T1.1 generates the corpus
THEN that shape SHALL be present, and T1.5 SHALL report its verdict against pan.

### Requirement: Verdict Gate
T1.2 SHALL complete BEFORE T1.5 SHALL run. The gate SHALL fail ALWAYS when a model is refuted that pan accepts, or accepted that pan refutes.

#### Scenario: false refutation
GIVEN a model that pan proves valid and we report with one error
WHEN T1.5 runs the gate
THEN the build SHALL fail and the model SHALL be named in the report.

### Requirement: Declines Are Counted
A model we do not decide SHALL be counted ALWAYS as a decline, never as an agreement.

#### Scenario: declined model
GIVEN a model carrying a property we decline to decide
WHEN T1.2 classifies the run
THEN the outcome SHALL be decline, and the total SHALL include it rather than omit it.

#### Scenario: agreement inflation
GIVEN a previously declined model whose error count happens to be zero
WHEN T1.2 classifies the run
THEN the outcome SHALL be decline and not agreement.

### Requirement: Nothing Leaves The Comparison Silently
A model whose oracle could not be built, and a model we decline, SHALL be named in every report ALWAYS. They are not wrong verdicts, but a corpus that loses models to them is a corpus that looks better than it is.

#### Scenario: oracle fails to build
GIVEN a model that pan refuses, for instance because two declarations share a name
WHEN the gate runs
THEN the model SHALL be listed as not compared, and the total SHALL include it.

### Requirement: Every Defect Class Is Covered
A defect found by hand SHALL be reachable by the generator BEFORE the fix is called complete, and the coverage SHALL be mutation-checked: reintroducing the defect SHALL make the gate fail.

#### Scenario: a loop body that is not atomic
GIVEN a `for` whose body writes twice, and a claim that observes the state between the writes
WHEN the loop is wrongly compiled as one atomic region
THEN a model in the corpus SHALL be a false clean and the gate SHALL fail.

### Requirement: Baseline Before Rewrite
T1.3 SHALL record the baseline BEFORE T2.1 SHALL run, so that the effect of the execution model is measured rather than claimed.

#### Scenario: baseline recorded
GIVEN the binary at the commit before the execution model changes
WHEN T1.3 has run
THEN the report SHALL state, per outcome, how many models fall in each class and SHALL name every wrong verdict.

### Requirement: Generator Self-Test
T1.4 SHALL complete BEFORE T1.5 SHALL run. The generator SHALL reproduce ALWAYS every shape known to be wrong, so that a grammar change which drops one fails the gate.

#### Scenario: generator regression
GIVEN the four shapes known at the baseline, being a two-statement body, an assignment before a loop, two sequential loops, and an assert in a blocked process
WHEN T1.4 runs
THEN each SHALL be generated, and a grammar change that drops one SHALL fail the gate.
