## Phase 1: Interactive Simulation

- [x] 1.1 Create `InteractiveSimulator` struct wrapping a model
- [x] 1.2 Implement step(): display current state, list enabled transitions, read user choice
- [x] 1.3 Implement step-back: store history of states for undo
- [x] 1.4 Implement state inspection: display variable values at current step
- [x] 1.5 Add `--interactive` CLI flag
- [x] 1.6 Add test: interactive simulation produces expected path

## Phase 2: Trail Replay with State Inspection

- [x] 2.1 Extend `TrailReplayer` to dump state at each step
- [x] 2.2 Add `--inspect` flag to trail replay for state dumps
- [x] 2.3 Support Spin-compatible trail format for reading
- [x] 2.4 Support Spin-compatible trail format for writing
- [x] 2.5 Add `-t` CLI option for trail replay
- [x] 2.6 Add `-k` CLI option for trail file
- [x] 2.7 Add test: trail replay with state inspection matches expected values

## Phase 3: Swarm Verification

- [x] 3.1 Design swarm config generation: N configs with varied seeds, hash functions, params
- [x] 3.2 Implement swarm runner: spawn N parallel workers via rayon
- [x] 3.3 Implement result merging: first violation wins, aggregate stats
- [x] 3.4 Add `--swarm N,M` CLI option
- [x] 3.5 Add test: swarm finds violation that single run misses

## Phase 4: Parallel BFS

- [x] 4.1 Add crossbeam and dashmap dependencies
- [x] 4.2 Implement concurrent BFS frontier (crossbeam channel)
- [x] 4.3 Implement shared visited set (dashmap)
- [x] 4.4 Implement parallel BFS worker: pop, expand, push
- [x] 4.5 Add `--bfspar` CLI option
- [x] 4.6 Add test: parallel BFS produces same results as sequential BFS

## Phase 5: Hash-Compact Storage

- [x] 5.1 Add `StorageMode::HashCompact` variant
- [x] 5.2 Implement hash-compact store: hash table of u64 + LRU cache
- [x] 5.3 Implement collision detection and fallback to exact storage
- [x] 5.4 Add `--hc` CLI option
- [x] 5.5 Add test: hash-compact storage produces same results as exact

## Phase 6: Strong Fairness

- [x] 6.1 Add per-transition enabled_count and taken_count to state vector
- [x] 6.2 Increment enabled_count for each transition in each state
- [x] 6.3 Increment taken_count when a transition is taken
- [x] 6.4 Implement strong fairness check during cycle detection
- [x] 6.5 Add `--strong-fairness` CLI option
- [x] 6.6 Add test: strong fairness detects violation that weak fairness misses
