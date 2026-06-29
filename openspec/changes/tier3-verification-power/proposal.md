## Why

spin-rs has basic DFS/BFS verification, but lacks the advanced verification modes that make Spin powerful: interactive simulation for debugging, trail replay with state inspection, swarm verification for large state spaces, parallel BFS for multi-core, hash-compact storage for memory efficiency, and strong fairness for liveness properties.

These features are essential for verifying real-world models where state spaces are large, bugs are subtle, and debugging requires step-by-step inspection.

## What Changes

### 1. Interactive Simulation

Spin's `-i` mode lets users step through a simulation interactively, choosing which transition to take at each step. This is invaluable for debugging models.

**Fix**: Add interactive mode to the CLI. At each step, display the current state and available transitions. Let the user pick one via stdin. Support step-back (undo) and state inspection.

### 2. Trail Replay with State Inspection

Current trail replay (`src/trail/mod.rs`) replays transitions but doesn't show intermediate states. Spin's `-t` and `-k` options replay trails with full state dumps.

**Fix**: Enhance trail replay to dump state at each step. Support Spin-compatible trail format. Add `-t` and `-k` CLI options.

### 3. Swarm Verification

Spin's `-swarm N,M` runs N randomized verification iterations in parallel, each with different hash functions and search parameters. This increases coverage for large state spaces.

**Fix**: Implement swarm mode: spawn N parallel verification workers, each with different random seeds, hash functions, and search parameters. Collect results from all workers.

### 4. Parallel BFS

Spin's `-bfspar` uses multiple cores for BFS exploration. Current spin-rs has a `parallel` feature with rayon but it's limited.

**Fix**: Implement parallel BFS using work-stealing: partition the BFS frontier across threads, use shared visited set with fine-grained locking.

### 5. Hash-Compact Storage

Spin's `-hc` stores states as 64-bit hashes instead of full state vectors, trading precision for memory. Useful for very large state spaces.

**Fix**: Implement hash-compact storage: store only the hash of each state, with a small cache of recent full states for collision detection.

### 6. Strong Fairness

Current weak fairness ensures each continuously enabled transition eventually executes. Strong fairness additionally ensures that transitions enabled infinitely often eventually execute.

**Fix**: Implement strong fairness tracking: for each transition, track whether it's enabled infinitely often. During cycle detection, check that fairly enabled transitions are taken.

## Capabilities

### New Capabilities

- `interactive-simulation`: Step-by-step interactive model execution with user choice
- `trail-replay`: Full trail replay with state inspection at each step
- `swarm`: Parallel randomized verification across multiple iterations
- `parallel-bfs`: Multi-core BFS state exploration
- `hash-compact`: Memory-efficient state storage using 64-bit hashes
- `strong-fairness`: Strong fairness constraints for liveness verification

### Modified Capabilities

- `verification-engine`: All verification modes (DFS, BFS, swarm, parallel) share a common interface
- `cli`: New CLI options for interactive mode, trail replay, swarm, parallel BFS
