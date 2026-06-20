## Why

spin-rs aims to replace the external Spin binary as the model checking backend for veriplan.
Before shipping this replacement, we need to know two things with confidence:

1. Does spin-rs produce correct results (same states, same violations, same trails) as Spin?
2. How much faster or slower is it, wall-clock, including Spin's GCC compilation step?

Without these numbers, swapping Spin for spin-rs in veriplan is blind.

## What Changes

- Build a benchmark suite under `benches/` that exercies both spin-rs and the reference Spin 6.5.x binary
- Measure **correctness equivalence** (Phase 1): same outputs on 15+ models across storage modes and search modes
- Measure **local performance** (Phase 2): breakdown of spin-rs's time budget (parse, codegen, Lua, serialize, hash)
- Measure **global performance** (Phase 3): wall-clock comparison — full pipeline and verification-only — against Spin
- Create a reusable model corpus (`.pml` files) spanning veriplan-like plan models, classic Spin protocols, and edge cases
- Ship a single `cargo run --bench --release` binary that outputs JSON + human-readable table

**Non-goals:**

- No channels in Phase 1 (deferred to later work)
- No changes to spin-rs's verification engine — this is measurement only
- No CI integration (out of scope)

## Capabilities

### New Capabilities

- `correctness-equivalence`: Compare spin-rs and Spin outputs state-for-state, violation-for-violation, across storage modes (exact, bitstate) and search modes (DFS, BFS). Defines pass/fail tolerance criteria.
- `local-performance`: Profile spin-rs's internal time budget — parse, codegen, Lua bootstrap, guard evaluation, effect execution, state serialization, hash lookup. Identifies bottlenecks.
- `global-comparison`: Wall-clock comparison of spin-rs vs Spin (full pipeline and verification-only) on the model corpus. Reports states/sec, transitions/sec, memory/state, and speedup factor.

### Modified Capabilities

None.

## Impact

- New `benches/` directory with benchmark harness, model corpus, and comparison logic
- New `Cargo.toml` dev-dependencies (criterion, pprof or similar)
- No changes to spin-rs library/CLI code
- Single benchmark binary: `cargo run --bench --release`
