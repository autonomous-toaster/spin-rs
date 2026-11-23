# Tasks

## Phase 1: The Instrument

- [x] 1.1 Build the structural model generator
- [x] 1.2 Classify every generated model against pan
- [x] 1.3 Record the baseline report before the execution model changes
- [x] 1.4 Assert that the generator reproduces every known wrong shape
- [x] 1.5 Fail the build on a wrong verdict
- [x] 1.6 Add the models shipped with spin as a second corpus

## Phase 2: Execution Model

- [x] 2.1 Build the per-proctype control-flow graph
- [x] 2.2 Emit one transition per edge
- [x] 2.3 Collapse the per-construct counters into one program counter
- [x] 2.4 Fuse an atomic region into one edge
- [x] 2.5 Compile control transfer to an edge target
- [x] 2.6 Compile a for loop to an exit edge
- [x] 2.7 Sweep sequencing against pan
- [x] 2.8 Remove a shape-specific repair that the graph makes unnecessary
- [ ] 2.9 Delete the old emitters
- [x] 2.10 Compile a nested do as a step
- [x] 2.11 Compile a for body as ordinary steps
- [x] 2.12 Fuse a d_step region into one step
- [x] 2.13 Compile a nested if
- [x] 2.14 Refuse an unencodable statement
- [x] 2.15 Absorb the optional trailing terminator in every statement parser
- [x] 2.16 Parse a for loop followed by a statement

## Phase 3: Statement Semantics

- [x] 3.1 Decide an else guard against the sibling guards
- [x] 3.2 Give an unless escape priority over the body
- [x] 3.2b Let an unless handler of several statements pause between them, on its own counter, with the body disabled while it can move
- [ ] 3.3 Enable timeout only when no process can move
- [x] 3.4 Enforce a provided clause on every transition
- [ ] 3.5 Enforce the d_step restrictions
- [ ] 3.6 Define deadlock as pan's invalid end state
- [ ] 3.7 Survey fairness against pan
- [ ] 3.8 Refuse run pending a measured answer
- [ ] 3.8a Put a process table in the state whose initial contents are the static instances
- [ ] 3.8b Allocate a pid from that table for `run`, binding its arguments at the call
- [ ] 3.8c Derive `_nr_pr` from the table rather than keeping a counter beside it
- [ ] 3.8d Check that every emitted arithmetic write to a declared field goes through `_spin_wrap`
- [x] 3.9 Filter the enabled set to the highest-priority process
- [x] 3.10 Parse a bare block
- [x] 3.11 Read a guard with Promela truthiness
- [x] 3.12 Absorb a statement separator

## Phase 4: Data Surface

- [x] 4.1 Truncate a stored value to the declared type
- [ ] 4.2 Put an array element in the state
- [ ] 4.3 Put a typedef field in the state
- [ ] 4.4 Resolve a remote reference
- [x] 4.5 Cover the channel surface
- [x] 4.6 Put a channel queue in the state

## Phase 5: Property Surface

- [ ] 5.1 Add Until to the formula type
- [ ] 5.2 Translate a nested temporal operator
- [x] 5.3 Evaluate an atom that is an expression
- [x] 5.4 Check a never claim as a monitor
- [ ] 5.5 Handle accept labels on a never claim
- [ ] 5.6 Reach liveness verdict parity on the corpus
- [x] 5.7 Stop reporting a terminating model as an accepting cycle

## Phase 6: Inert Mechanism Audit

- [ ] 6.1 Audit an AST field for participation
- [ ] 6.2 Fail a test when a state key is read by nothing
