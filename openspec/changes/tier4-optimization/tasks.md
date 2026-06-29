## Phase 1: Dataflow Analysis

- [ ] 1.1 Design GEN/KILL set computation for each transition
- [ ] 1.2 Implement GEN set: variables read by guard expression
- [ ] 1.3 Implement KILL set: variables written by effect
- [ ] 1.4 Implement fixed-point iteration over control-flow graph
- [ ] 1.5 Compute IN/OUT sets for each transition
- [ ] 1.6 Add test: dataflow analysis produces correct GEN/KILL for simple model
- [ ] 1.7 Add test: dataflow analysis handles loops correctly

## Phase 2: Dead Variable Elimination

- [ ] 2.1 Implement liveness analysis using dataflow IN/OUT sets
- [ ] 2.2 Mark variables as dead if written but never subsequently read
- [ ] 2.3 Remove dead variables from state vector initialization
- [ ] 2.4 Skip dead variable assignments in effect codegen
- [ ] 2.5 Add `-o2` CLI flag
- [ ] 2.6 Add test: dead variable excluded from state vector
- [ ] 2.7 Add test: live variables are not affected

## Phase 3: Statement Merging

- [ ] 3.1 Implement mergeability check: deterministic, no blocking, no channel ops
- [ ] 3.2 Implement merge pass: combine consecutive mergeable transitions
- [ ] 3.3 Handle merged transition guard: AND of both guards
- [ ] 3.4 Handle merged transition effect: sequence of both effects
- [ ] 3.5 Add `-o3` CLI flag
- [ ] 3.6 Add test: merged transitions produce same verification result
- [ ] 3.7 Add test: non-mergeable transitions are not merged

## Phase 4: Rendezvous Optimization

- [ ] 4.1 Detect sync channels (capacity 0) during codegen
- [ ] 4.2 Detect send followed by receive on same sync channel
- [ ] 4.3 Merge sync send/recv pair into single transition
- [ ] 4.4 Add `-o4` CLI flag
- [ ] 4.5 Add test: rendezvous optimization reduces states
- [ ] 4.6 Add test: async channels are not affected
