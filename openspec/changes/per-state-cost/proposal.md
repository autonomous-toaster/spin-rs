# Per State Cost

Carries kata `a6fr` ("Per-state cost is ~100x pan even when state counts agree"). It was the phase-4
tail of `veriplan-perf-parity` and is split out because the floor it was measured against cannot be
measured on the fixture it names.

## Why

`veriplan-perf-parity` fixed what made the consumer's plans unusable: the state count is linear on
their shape (97 states at N=12 against pan's 73) and the liveness path is bounded. What remains is
that no caller can run a wide model for long: the engine pays ~0.70 ms per state at 422 blob
entries, where pan pays tens of microseconds for a state it hashes in place.

## Problem

Measured at v0.5.42, release binary, pan 6.5.2 on PATH. Not estimates.

| what | figure |
|---|---|
| startup, tiny model, 5 runs | 3-4 ms |
| N=60 chain, 481 states / 599 transitions / 422 blob entries | 0.336 s = **1432 states/s**, 0.70 ms/state |
| `bench_split` at 422 entries | expansion **588 us**, `project_blob` 41 us, hash 0.0 us |
| the change's own field scaling | 141 us at 72 fields, 293 at 142, 566 at 282 - ~2 us per entry |

So cost tracks **entries per state**, at ~1.6 us each, and it is not the string projection (0.10 us
per entry). It is, per entry: the blob parse into a Lua table, a deep clone of the whole table for
every enabled transition, a full reserialization of every successor, and N proctype calls per state
that each allocate guard and effect closures - including the N-1 that are blocked.

Two things follow, and they are the reason this change starts with an instrument rather than a fix.

1. **The old floor was unmeasurable.** 1e5 states/s on the N=8 chain means 65 states in 0.65 ms;
   startup alone is 3-4 ms. The fixture's ceiling is ~16k states/s with a free engine.
2. **The old cost model was inverted.** "A structured state would attack the 40 us, not the
   991 us" compared `project_blob` alone against an expansion that already contained the parse,
   the clone and the serialize. The 1.6 us per entry is those, so the named levers were never
   priced against the work they target. Task 1.3 is that price.

## What changes

1. **The instrument first.** Commit the throughput fixture, replace the bench that reads
   `/tmp/c60.pml`, and time the expansion by section: parse, step-table construction, clone,
   effect, serialize. Report us per entry and per transition, not only states/s.
2. **The transition table**, once per model instead of once per state (the half of 4.2 that was
   never done: closures are still allocated per state, for blocked processes too).
3. **The state representation** at the transition boundary: no deep clone per enabled transition,
   no reserialization per successor, full-state comparison still in the store.

## Non-goals

- Bitstate or hash-only storage. Unchanged: the store keeps comparing full states.
- Compiling the transition relation to Rust. If the consumer needs 1e5 states/s, that is the route,
  and it is a different change: it replaces the Lua backend rather than cutting its per-entry cost.
  This change records the figure it does reach and does not present it as pan parity.
- Changing the state projection or the liveness budget. Both are settled in
  `veriplan-perf-parity` and this change must not move a count they pinned.

## Task map

| Task | What it does |
|---|---|
| T1.1 | Commits the throughput fixture (481 states, 599 transitions, 422 entries) |
| T1.2 | Fixes the bench that reads an uncommitted `/tmp` file |
| T1.3 | Splits the expansion into parse, step table, clone, effect and serialize |
| T1.4 | Reports states/s, us/state, us/entry, us/transition and the startup share |
| T1.5 | Records the baseline for both transition densities |
| T2.1 | Builds each proctype's step table once per model |
| T2.2 | Binds the state as an argument instead of capturing it in a closure |
| T2.3 | Skips closure and descriptor allocation for blocked processes |
| T2.4 | Measures phase 2 and sweeps the corpus |
| T3.1 | Serializes at the store boundary, not per successor |
| T3.2 | Copies the write set, not the state |
| T3.3 | Keeps the store comparing full states |
| T3.4 | Keeps the projection claim-relative, applied before hashing and storing |
| T3.5 | Measures phase 3 and sweeps the corpus |
| T4.1 | Asserts the floor in a release build |
| T4.2 | Asserts per-entry cost does not grow with transition density |
| T4.3 | Reports the achieved figure with its fixture and density |
| T5.1 | Sweeps the corpus for verdicts |
| T5.2 | Asserts the chain probe counts |

## Gate

1432 states/s measured before, at least 5700 after, on a committed fixture of at least 400 states,
with the chain probe counts unchanged at 49/65/81/97 and every corpus verdict unchanged.
