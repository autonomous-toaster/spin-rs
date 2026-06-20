## Why

The benchmark suite has 12 models, but only `single_loop` produces state counts close to Spin (102 vs 103). The rest fall into three categories:

1. **Channels & deadlock** (`deadlock_circular`, `token_ring_n5`): Skipped entirely because rendezvous channels and channel buffering aren't implemented. `deadlock_circular` is the highest-priority gap since deadlock detection is a core verification feature.

2. **Parser gaps** (`plan_5tasks_3ltls`, `plan_20tasks_10ltls`, `peterson_n2/n3`, `dining_n4`, `state_explosion`): Most of these have 1 state due to parser not supporting multi-variable declarations (`bool a, b, c;`), `active [N] proctype`, `_pid`, `inline`, or `for` loops. The generated Lua code references undefined variables, causing the guard to evaluate to nil → false → 0 transitions.

3. **LTL in benchmark** (`ltl_violation`): The benchmark calls `verify()` which only checks safety assertions, not LTL. `verify_ltl()` is a separate function that isn't wired into the benchmark pipeline.

## What Changes

### Phase 1 — Channels & Deadlock Detection (highest priority)

- Parse `chan ch = [N] of { byte }` correctly into `TopLevel::ChanDecl`
- Emit channel state in `_spin_init_state` so it appears in the state blob
- Wire up channel declarations in the runtime (currently dead code that checks for `ChanDecl`)
- Implement deadlock detection in `check_violation()` for `LuaModel`
- Test with `deadlock_circular` model (expects 1 error)

### Phase 2 — Parser Completeness

- Add multi-variable declarations: `bool a, b, c;` → multiple `VarDecl`
- Add `active [N] proctype name()` → expand to N process instances
- Add `_pid` as a built-in variable (per-process ID)
- Add `init { }` block support
- Add `inline` definition and expansion
- Add `for` loop parsing (`for (i in 0 .. 4) { }`)
- Add `else` guard support (should always be last choice)

### Phase 3 — Codegen & Benchmark

- Wire `verify_ltl()` into the benchmark comparison pipeline
- Fix codegen for `_pid` references
- Add proper modeline/embedded C state variable extraction
- Improve state count accuracy for multi-proctype models

## Impact

- `src/parser/mod.rs`: Major additions — multi-decl, active[N], inline, for, init, _pid
- `src/codegen/mod.rs`: Channel state in init, _pid support, inline expansion
- `src/runtime/mod.rs`: Channel declaration wiring, deadlock detection
- `src/bin/bench_vs_spin.rs`: LTL verification, remove skipped models
