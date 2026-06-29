## Phase 1: Interactive Simulation

- [ ] 1.1 Create `InteractiveSimulator` struct wrapping a model
- [ ] 1.2 Implement step(): display current state, list enabled transitions, read user choice
- [ ] 1.3 Implement step-back: store history of states for undo
- [ ] 1.4 Implement state inspection: display variable values at current step
- [ ] 1.5 Add `--interactive` CLI flag
- [ ] 1.6 Add test: interactive simulation produces expected path

## Phase 2: Trail Replay with State Inspection

- [ ] 2.1 Extend `TrailReplayer` to dump state at each step
- [ ] 2.2 Add `--inspect` flag to trail replay for state dumps
- [ ] 2.3 Support Spin-compatible trail format for reading
- [ ] 2.4 Support Spin-compatible trail format for writing
- [ ] 2.5 Add `-t` CLI option for trail replay
- [ ] 2.6 Add `-k` CLI option for trail file
- [ ] 2.7 Add test: trail replay with state inspection matches expected values

## Phase 3: Swarm Verification

- [ ] 3.1 Design swarm config generation: N configs with varied seeds, hash functions, params
- [ ] 3.2 Implement swarm runner: spawn N parallel workers via rayon
- [ ] 3.3 Implement result merging: first violation wins, aggregate stats
- [ ] 3.4 Add `--swarm N,M` CLI option
- [ ] 3.5 Add test: swarm finds violation that single run misses

## Phase 4: Parallel BFS

- [ ] 4.1 Add crossbeam and dashmap dependencies
- [ ] 4.2 Implement concurrent BFS frontier (crossbeam channel)
- [ ] 4.3 Implement shared visited set (dashmap)
- [ ] 4.4 Implement parallel BFS worker: pop, expand, push
- [ ] 4.5 Add `--bfspar` CLI option
- [ ] 4.6 Add test: parallel BFS produces same results as sequential BFS

## Phase 5: Hash-Compact Storage

- [ ] 5.1 Add `StorageMode::HashCompact` variant
- [ ] 5.2 Implement hash-compact store: hash table of u64 + LRU cache
- [ ] 5.3 Implement collision detection and fallback to exact storage
- [ ] 5.4 Add `--hc` CLI option
- [ ] 5.5 Add test: hash-compact storage produces same results as exact

## Phase 6: Strong Fairness

- [ ] 6.1 Add per-transition enabled_count and taken_count to state vector
- [ ] 6.2 Increment enabled_count for each transition in each state
- [ ] 6.3 Increment taken_count when a transition is taken
- [ ] 6.4 Implement strong fairness check during cycle detection
- [ ] 6.5 Add `--strong-fairness` CLI option
- [ ] 6.6 Add test: strong fairness detects violation that weak fairness misses
