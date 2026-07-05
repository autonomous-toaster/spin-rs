## Context

The codegen in `src/codegen/` currently emits one transition per statement. Optimizations are applied as a post-processing pass on the generated transition list before emitting Lua. Each optimization is controlled by a flag and can be enabled/disabled independently.

## Goals / Non-Goals

**Goals:**

- Statement merging reduces intermediate states by 20-50% on typical models
- Dead variable elimination reduces state vector size
- Dataflow analysis enables better optimization decisions
- Rendezvous optimization reduces states for sync channel models
- All optimizations are opt-in via `-o1` through `-o4` flags
- All existing tests pass with optimizations enabled

**Non-Goals:**

- Full Spin-compatible optimization levels (no `-o5` case caching)
- Runtime optimization of Lua execution
- Automatic optimization level selection

## Decisions

**Decision 1: Optimizations as codegen post-processing**

After generating the initial transition list for a proctype, apply optimization passes in sequence:

1. Dataflow analysis (computes GEN/KILL sets)
2. Dead variable elimination (removes dead vars from state vector)
3. Statement merging (combines consecutive deterministic transitions)
4. Rendezvous optimization (merges sync send/recv pairs)

Each pass is optional and controlled by a flag.

**Decision 2: Mergeability criteria**

Two consecutive statements A then B are mergeable if:

- A is deterministic (no non-deterministic choice)
- B is deterministic
- A's effect doesn't affect B's guard in a way that creates new non-determinism
- Neither A nor B is a channel operation (unless rendezvous optimization is enabled)

Merged transition: guard = A.guard AND B.guard, effect = A.effect then B.effect.

**Decision 3: Dead variable detection via liveness analysis**

A variable is dead at a program point if it's never read after that point. Liveness analysis is backward: start from the end of the proctype and track which variables are needed. Variables that are written but never read are removed from the state vector.

**Decision 4: Dataflow analysis as GEN/KILL sets**

Each transition has:

- GEN: variables read by the guard
- KILL: variables written by the effect
- IN: variables live on entry
- OUT: variables live on exit

These sets are computed by a fixed-point iteration over the control-flow graph.

**Decision 5: Rendezvous pair detection**

A rendezvous send `ch!val` followed by a receive `ch?var` on the same sync channel (capacity 0) can be merged. Detection is syntactic: look for consecutive transitions where the first is a send on a sync channel and the second is a receive on the same channel.

## Risks / Trade-offs

- **Statement merging correctness**: Merging changes the state space structure. Must ensure merged transitions don't hide non-determinism
- **Dead variable elimination**: Removing variables changes state hashes. Must ensure hash consistency
- **Dataflow analysis cost**: Fixed-point iteration adds codegen time but is negligible compared to verification
- **Rendezvous optimization**: Only applies to sync channels (capacity 0). Async channels are not affected
