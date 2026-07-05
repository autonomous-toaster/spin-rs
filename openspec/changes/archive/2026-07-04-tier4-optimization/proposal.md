## Why

The generated Lua verifier currently has no optimizations. Every statement produces a separate transition, every variable is stored in the state vector, and no dataflow analysis is performed. This leads to larger state spaces, more transitions per state, and slower verification than necessary.

Spin's C verifier has four key optimizations controlled by `-o1` through `-o4` flags. These reduce state space size, transition count, and verification time without affecting correctness.

## What Changes

### 1. Statement Merging (`-o3`)

When multiple consecutive statements are deterministic (no non-deterministic choice, no blocking), they can be merged into a single transition. This reduces the number of intermediate states.

**Fix**: During codegen, analyze statement sequences for mergeability. If statements A then B are both deterministic and B's guard doesn't depend on A's effect in a way that creates new non-determinism, merge them into one transition with combined guard and effect.

### 2. Dead Variable Elimination (`-o2`)

Variables that are written but never read can be eliminated from the state vector. This reduces state vector size and memory usage.

**Fix**: During codegen, perform liveness analysis on each proctype's variables. Variables that are written but never subsequently read are marked as dead and excluded from the state vector.

### 3. Dataflow Analysis (`-o1`)

Dataflow analysis tracks which variables are read and written by each transition. This enables better merging decisions and dead variable detection. It also enables the "write-only" optimization where variables written before any read don't need initial values.

**Fix**: Implement a dataflow analysis pass that computes GEN/KILL sets for each transition. Use this information to improve statement merging and dead variable elimination.

### 4. Rendezvous Optimization (`-o4`)

Rendezvous channels (capacity 0) synchronize sender and receiver in one atomic step. The optimization recognizes that a rendezvous send followed by a rendezvous receive on the same channel can be combined into a single transition, reducing intermediate states.

**Fix**: During codegen, detect rendezvous send/receive pairs. When a send on a sync channel is immediately followed by a receive on the same channel, merge them into a single transition.

## Capabilities

### New Capabilities

- `stmt-merging`: Deterministic statement sequences are merged into single transitions
- `dead-var-elim`: Unused variables are excluded from the state vector
- `dataflow`: GEN/KILL analysis enables better optimization decisions
- `rendezvous-opt`: Rendezvous send/receive pairs are merged into single transitions

### Modified Capabilities

- `codegen-core`: All optimizations are applied during codegen, producing smaller/faster Lua
- `cli`: New `-o1` through `-o4` flags to control optimizations
