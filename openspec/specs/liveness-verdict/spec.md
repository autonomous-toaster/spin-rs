# liveness-verdict Specification

## Purpose
TBD - created by archiving change liveness-soundness. Update Purpose after archive.

## Requirements

### Requirement: Negated Property Automaton
T1.1 SHALL complete BEFORE T1.4 SHALL run. The product search SHALL judge a property ALWAYS against the automaton for its negation, so that an accepting cycle means the property is violated.

#### Scenario: T1.1 satisfied liveness property
GIVEN a model where p becomes true and stays true
WHEN T1.1 checks `<>p`
THEN no violation SHALL be reported.

### Requirement: Seed-Anchored Accepting Cycles
T1.2 SHALL complete BEFORE T1.3 SHALL run. A cycle SHALL be reported ALWAYS only when it returns to the accepting state that seeded the inner search.

#### Scenario: T1.2 cycle inside a rejecting state
GIVEN a product run that loops entirely inside a rejecting automaton state
WHEN T1.2 runs
THEN no violation SHALL be reported.

### Requirement: No Verdict Without Evaluation
T1.3 SHALL complete BEFORE T2.2 SHALL run. The verifier SHALL NOT report a verdict ALWAYS for a property it did not evaluate; an unsupported property SHALL be an error.

#### Scenario: T1.3 unsupported property
GIVEN a property the automaton builder cannot complement
WHEN T1.3 runs
THEN the check SHALL fail with a diagnostic naming the property.

### Requirement: BFS Checks Properties
T2.1 SHALL complete AFTER T1.1. Breadth-first search SHALL evaluate ALWAYS every LTL property that depth-first search evaluates.

#### Scenario: T2.1 bfs liveness
GIVEN a model that violates `<>p`
WHEN T2.1 runs with breadth-first search
THEN a violation SHALL be reported.

### Requirement: Engines Refuse Unevaluated LTL
T2.2 SHALL complete BEFORE T3.1 SHALL run. An engine that does not evaluate LTL SHALL refuse ALWAYS to run on a model declaring it.

#### Scenario: T2.2 swarm with ltl
GIVEN a model declaring one ltl property
WHEN T2.2 runs with the swarm engine
THEN the run SHALL fail with a diagnostic and no verdict.

### Requirement: Parallel BFS Terminates
T3.1 SHALL complete BEFORE T3.2 SHALL run. Parallel BFS SHALL terminate ALWAYS once no items are queued and no worker is active.

#### Scenario: T3.1 drained frontier
GIVEN a parallel BFS run whose frontier has drained
WHEN T3.1 runs
THEN the worker threads SHALL exit and the result SHALL be returned.
