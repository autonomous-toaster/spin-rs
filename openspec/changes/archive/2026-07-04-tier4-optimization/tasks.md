## Phase 1: Dataflow Analysis

- [x] 1.1 Design GEN/KILL set computation for each transition
- [x] 1.2 Implement GEN set: variables read by guard expression
- [x] 1.3 Implement KILL set: variables written by effect
- [x] 1.4 Implement fixed-point iteration over control-flow graph
- [x] 1.5 Compute IN/OUT sets for each transition
- [x] 1.6 Add test: dataflow analysis produces correct GEN/KILL for simple model
- [x] 1.7 Add test: dataflow analysis handles loops correctly

## Phase 2: Dead Variable Elimination

- [x] 2.1 Implement liveness analysis using dataflow IN/OUT sets
- [x] 2.2 Mark variables as dead if written but never subsequently read
- [x] 2.3 Remove dead variables from state vector initialization
- [x] 2.4 Skip dead variable assignments in effect codegen
- [x] 2.5 Add `-o2` CLI flag
- [x] 2.6 Add test: dead variable excluded from state vector
- [x] 2.7 Add test: live variables are not affected

## Phase 3: Statement Merging

- [x] 3.1 Implement mergeability check: deterministic, no blocking, no channel ops
- [x] 3.2 Implement merge pass: combine consecutive mergeable transitions
- [x] 3.3 Handle merged transition guard: AND of both guards
- [x] 3.4 Handle merged transition effect: sequence of both effects
- [x] 3.5 Add `-o3` CLI flag
- [x] 3.6 Add test: merged transitions produce same verification result
- [x] 3.7 Add test: non-mergeable transitions are not merged

## Phase 4: Rendezvous Optimization

- [x] 4.1 Detect sync channels (capacity 0) during codegen
- [x] 4.2 Detect send followed by receive on same sync channel
- [x] 4.3 Merge sync send/recv pair into single transition
- [x] 4.4 Add `-o4` CLI flag
- [x] 4.5 Add test: rendezvous optimization reduces states
- [x] 4.6 Add test: async channels are not affected
