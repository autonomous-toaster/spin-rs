## Why

The benchmark suite has made significant progress (1→10-233 states on complex models), but three critical gaps block full equivalence with upstream Spin:

1. **Channel syntax parser** - `chan ch = [0] of { byte };` doesn't parse, breaking `deadlock_circular` and `token_ring_n5` models
2. **LTL verification** - `verify_ltl()` exists but isn't wired into benchmark, breaking `ltl_violation` model
3. **Boolean expression regression** - `~= 0` check applied everywhere broke `single_loop` (102→2 states)

Without these fixes, spin-rs cannot replace Spin as veriplan's verification backend.

## What Changes

### Phase 1 — Channel Syntax Parser (BLOCKING)

- Add parser rule for `chan name = [N] of { type };` syntax
- Extract capacity and message type from channel declarations
- Generate proper channel state in `_spin_init_state`
- Test with `deadlock_circular` model (expects 1 error)

### Phase 2 — LTL Integration (BLOCKING)

- Wire `verify_ltl()` into benchmark for models with LTL formulas
- Detect LTL formulas in parsed models
- Compare spin-rs LTL results against Spin's error counts
- Test with `ltl_violation` model (expects 1 error)

### Phase 3 — Boolean Expression Scoping (REGRESSION FIX)

- Distinguish guard context vs assignment context in codegen
- Apply `~= 0` check ONLY in guard/condition expressions
- Remove `~= 0` from assignment expressions
- Validate `single_loop` returns to 102+ states

## Impact

- `src/parser/mod.rs`: New parser rule for channel syntax
- `src/codegen/mod.rs`: Context-aware boolean checks (guard vs assignment)
- `src/bin/bench_vs_spin.rs`: LTL verification integration
- `benches/`: Remove skipped models (`deadlock_circular`, `token_ring_n5`)

## Capabilities

### New Capabilities

- `channel-syntax`: Parse `chan name = [N] of { type }` declarations
- `ltl-benchmark`: LTL verification in benchmark comparison
- `boolean-scoping`: Context-aware `~= 0` checks in Lua codegen

### Modified Capabilities

- `deadlock-detection`: Now testable once channels parse correctly
- `state-exploration`: Fixed regression in `single_loop` model

## Non-Goals

- No channel array enhancements (separate change)
- No multi-field message support (deferred)
- No performance optimizations (focus on correctness)
