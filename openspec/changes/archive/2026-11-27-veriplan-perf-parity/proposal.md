# Veriplan Performance Parity

## Why

A consumer with a realistic workload cannot use the engine in-process, and the reason is structural rather than a constant factor: the state count grows as 2^N where pan's is linear, the liveness search has no budget at all so a caller's fuse cannot bound it, and per-state cost is ~100x pan. Fixing only the speed leaves the blowup; fixing only the blowup leaves a hang. The three are filed and measured together because no single one of them makes the consumer's workload runnable.

## Problem

A consumer (veriplan) generates one active proctype per plan task and one ltl claim per
requirement, then verifies the result in-process. On its realistic shape the engine is
unusable, and the reason is not a constant factor. Three defects, measured at v0.5.35
(`b31e386`) on the consumer's own models, which are committed as fixtures here.

The shape: a chain of N active proctypes, each carrying `if :: failed_tN = 1; :: skip; fi;`,
with one claim that does not read `failed_*`.

| N | spin-rs states | pan states | spin-rs verdict |
|---|---|---|---|
| 6 | 694 | 37 | clean |
| 8 | 2806 | 49 | clean |
| 10 | 11254 | 61 | clean |

spin-rs doubles per task; pan is linear. Fitted, spin-rs is `~2^N`, pan is `~6N+1`. At N=24
that is ~16.7M states per claim. The consumer's 24-task, 18-claim plan exceeded 300 s where pan
finished in 1.0 s.

1. **No claim-relative state projection.** `LuaModel::hash` (src/runtime/mod.rs:474) hashes the
   whole `StateBlob`, so a bit the selected claim cannot read still distinguishes two states.
   Reduction exists only for *transitions* (`ReductionScope.visible` -> ample sets), never for
   *states*. pan's dead-variable analysis has no counterpart here.
2. **The liveness path has no budget.** `NestedDFS` holds `visited1`/`visited2` and an
   `Option<usize> max_depth` that no caller sets. `PropertyChecker::check_liveness`
   (src/property/mod.rs:380) and `check_claim` (:417) construct it without one. `--max-states 1`
   on a model with one `<>` claim still runs past 45 s.
3. **Per-state cost ~100x pan.** 2806 states in 0.412 s = ~6.8k states/s against pan's 1e5-1e6.
   `enumerate_transitions` compiles a Lua chunk per state visit, allocates two closures per
   process per state, deep-copies the table per enabled transition, and stores every state as an
   owned `String`.

## Why the verdict comparison alone does not catch 1

The obvious probe - the consumer's own claim `[] ( active_X -> done_Y )` - is refuted by both
engines, but pan refutes it in **3 states** and we refute it in **11254**. Agreement on the
verdict hides a 3750x cost gap and an early-refutation asymmetry. The state-count property is
therefore measured on a claim that does **not** refute, so the space must close.

## Task map

| Capability | Tasks |
|---|---|
| Instrument / probe discipline | 1.1-1.6 |
| Claim-relative state projection | 2.1-2.11 |
| Liveness budget | 3.1-3.7 |
| Per-state throughput | 4.1-4.4 (4.3, 4.5 moved to `per-state-cost`) |

## Non-goals

- Bitstate/hashing-only storage as a route to this win. `StorageMode::HashCompact` exists but
  trades completeness for speed and cannot be the basis of a state-count claim.
- Changing how pan's per-claim traversal is modelled here. Comparison stays a sum over the
  per-claim traversals a caller of pan would actually pay, as established in
  `archive/2026-11-22-partial-order-reduction`.
- Fairness (`-f`). Unchanged and out of scope.
- **Per-state cost itself**, since the correction recorded in `tasks.md`. The floor this proposal
  states (1e5 states/s on the N=8 fixture) cannot be measured on that fixture: it explores 65
  states and process startup alone is 3-4 ms, so the fixture's ceiling is ~16k states/s at any
  engine speed. The work that would move the cost is carried by `openspec/changes/per-state-cost`,
  which also restates the floor on a fixture that can carry it.

## What changes

1. Project the state onto the claim's observable fields before hashing and before storing, per
   claim. The projection MUST retain every field `check_violation` can read for the same run.
2. Give `NestedDFS` a state ceiling and route exhaustion to `Unverified{StateBudget}` naming the
   claim, so the per-claim invariant in src/engine/checker/mod.rs holds for liveness too.
3. Remove the per-state Lua compile and the Rust-side per-transition crossings (done: 4.1, 4.2).
   The remaining per-state cost - the per-transition deep clone and reserialization, and the
   transition table rebuilt for every state - is `per-state-cost`.

Order is fixed: 1 and 2 change what a caller can conclude; 3 is throughput and is gated behind
them, which is why this change closes on 1 and 2 and hands 3 forward.

## Consumer agreement

The consumer's acceptance criteria are adopted with two amendments, both measured rather than
argued: AC1 is restated on a non-refuting claim (their stated probe is refuted in 3 states by
both engines and does not exercise the property), and AC3 is stated as a floor with the
measurement method pinned, because throughput numbers drift with the machine.
