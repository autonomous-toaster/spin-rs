# Tasks

## Phase 1: Liveness Verdict

- [x] 1.1 Build the negated automaton for supported properties, error otherwise
- [x] 1.2 Report a cycle only when it returns to the accepting seed
- [x] 1.3 Delete the any-cycle fallback
- [x] 1.4 Regression test: `<>p` violated and `<>p` satisfied give opposite verdicts

## Phase 2: Engine Coverage

- [x] 2.1 Run the LTL property check under `--search bfs`
- [x] 2.2 Refuse `--swarm` and `--bfspar` on models declaring LTL

## Phase 3: Parallel BFS Termination

- [x] 3.1 Terminate parallel BFS on pending/active counters
- [x] 3.2 Its test suite completes instead of hanging
