## Context

The verification engine in `src/engine/` has a `Checker` struct with DFS/BFS methods. The `Model` trait defines the interface. New verification modes need to implement the same trait but with different exploration strategies.

## Goals / Non-Goals

**Goals:**

- Interactive simulation for debugging models
- Trail replay with full state inspection
- Swarm verification for large state spaces
- Parallel BFS using multiple cores
- Hash-compact storage for memory efficiency
- Strong fairness for liveness properties

**Non-Goals:**

- GUI for interactive simulation (CLI-only)
- Distributed verification across machines
- Spin-compatible trail format for all edge cases

## Decisions

**Decision 1: Interactive simulation as a separate binary mode**

Interactive mode is fundamentally different from batch verification. It runs a single path (not full state space exploration) and pauses at each step for user input. Implement as a separate `InteractiveSimulator` struct that wraps the model and provides step-by-step control.

**Decision 2: Trail replay reuses the existing TrailReplayer**

The existing `TrailReplayer` in `src/trail/mod.rs` is extended to dump state at each step. A new `--inspect` flag enables state inspection during replay. The Spin-compatible trail format is supported for reading/writing.

**Decision 3: Swarm as a parallel iterator over CheckerConfigs**

Swarm creates N `CheckerConfig` variants with different random seeds, hash functions, and search parameters. Each runs in its own thread via rayon. Results are merged: first violation found wins, or all results are aggregated.

**Decision 4: Parallel BFS with shared visited set**

The BFS frontier is a concurrent queue (crossbeam). Each worker pops a state, generates transitions, and pushes new states to the frontier. The visited set uses a concurrent hashmap (dashmap) with fine-grained locking. Workers are spawned via rayon.

**Decision 5: Hash-compact as a new StateStore variant**

Add `StorageMode::HashCompact` to the existing storage enum. The store keeps a hash table of 64-bit hashes and a small LRU cache of recent full states for collision detection. On collision (same hash, different state), fall back to exact storage for that state.

**Decision 6: Strong fairness via per-transition counters**

Each transition gets an `enabled_count` and `taken_count` in the state vector. During cycle detection, a transition is "fairly enabled" if it's enabled infinitely often (enabled_count grows unbounded). A transition is "fairly taken" if taken_count >= enabled_count in the limit. Strong fairness requires that every fairly enabled transition is fairly taken.

## Risks / Trade-offs

- **Interactive mode**: Requires stdin handling that may conflict with batch mode. Use a separate subcommand
- **Swarm overhead**: N parallel workers need N times the memory. Each worker has its own visited set
- **Parallel BFS**: Shared visited set creates contention. Fine-grained locking helps but doesn't eliminate it
- **Hash-compact collisions**: 64-bit hash collisions are rare (birthday bound at ~4B states) but possible. The LRU cache mitigates this
- **Strong fairness**: Adds per-transition counters to state vector, increasing size. Only needed for liveness verification
