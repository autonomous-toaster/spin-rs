## Why

The benchmark suite has 12 models, yet only 6 of 36 benchmark configurations pass. Root cause: 9 distinct bugs across the codegen, runtime, and engine layers prevent most models from exploring more than 1-2 states. Deadlock false positives mask real errors, arrays collapse to scalars, local variable writes target the wrong Lua keys, LTL verification never runs, and inline definitions are never expanded. Fixing these bugs gets the benchmark to ~30/36 pass and makes spin-rs a working model checker for most Promela models.

## What Changes

### Phase 1 — Local Variable Prefix in Codegen (P0)

Guard body effects in `emit_guards` write to `s.x` (bare variable name) instead of `s.P_x` (proctype-prefixed). The init state layout correct prefixes, and `emit_assignment` (standalone transition version) also prefixes correctly — but the inline path inside guard bodies does not.

- Fix the inline assignment effect in `emit_guards` to prefix targets with `current_proctype`
- Fix `emit_assignment_effect` (used in atomic/d_step blocks) to prefix targets

**Impacted models**: single_loop, multi_process, state_explosion, plan_5tasks_3ltls, plan_20tasks_10ltls — essentially every model with local variable assignments inside do/od blocks.

### Phase 2 — Deadlock False Positive (P0)

`check_violation` flags deadlock when `nr_pr >= 2` and the state blob contains both `:false` and `_done_`. This fires on normal sequential completion (first process finishes, others still running).

- Check each process's `_done_<name>` flag individually
- Only flag deadlock when at least one process has `_done == false` AND that process has zero transitions
- Add `nr_pr` tracking that counts only processes where `_done == false`

**Impacted models**: multi_process, plan_5tasks_3ltls, plan_20tasks_10ltls — these report 1 error (deadlock false positive) instead of 0.

### Phase 3 — Array Initialization (P1)

Arrays like `byte flag[2]` initialize as `state.flag = 0` (scalar). Array access in guards (`flag[1-_pid]`) then reads `nil` from the scalar, producing `false` guads.

- Emit arrays as Lua tables: `state.flag = {0, 0}` for `byte flag[2]`
- Handle multi-dimensional arrays and nested arrays in expressions

**Impacted models**: peterson_n2, peterson_n3 (both use `byte flag[2/3]`), state_explosion (uses scalar vars only but benefits from correct array handling)

### Phase 4 — For Loop Expansion (P1)

`for (i in 0 .. 4) { body }` generates incorrect Lua. The codegen's `Stmt::For` branch comments out the loop body and emits sequential stmts in the wrong context — the loop variable never actually iterates.

- Expand `for` loops at parse time into sequential statements (simple approach, covers all benchmark uses)
- Or fix the codegen to emit correct Lua iteration with a loop variable

**Impacted models**: token_ring_n5 (uses `for` inside init block to initialize channels)

### Phase 5 — LTL Verification Wiring (P2)

`check_ltl_properties` in `Checker::check_dfs` is a no-op stub. The property engine has a working `PropertyChecker`, `ltl2ba` simplified, `NestedDFS`, and `ProductState` — but none of it is wired into the main verification pipeline.

- Wire `verify_ltl()` into the `Checker::check()` path for models with LTL formulas
- Build product automaton (model × ¬LTL) and run nested DFS
- Report LTL violations alongside safety violations

**Impacted models**: ltl_violation (expects 1 error), plan_5tasks_3ltls, plan_20tasks_10ltls (both have LTL formulas)

### Phase 6 — Inline Expansion (P2)

`inline` definitions are parsed and stored in the AST (`TopLevel::Inline` with `InlineDef`) but never expanded at call sites. The codegen emits a comment. When a guard references an inline function, it evaluates to `nil` in Lua → always false.

- Implement inline expansion at codegen time: for each inline call, substitute parameters and emit the body
- Handle `inline pickup(i) { atomic { (fork[i] == 0); fork[i] = 1 } }`
- Handle `inline putdown(i) { fork[i] = 0 }`

**Impacted models**: dining_n4 (uses inline for pickup/putdown)

### Phase 7 — Rendezvous Channel Semantics (P3)

Capacity-0 channels (`chan ch = [0] of { byte }`) should be rendezvous: send blocks until a matching recv. Currently, `LuaChannel::send` checks `capacity > 0 && messages.len() >= capacity` — so capacity 0 always allows sends through.

- Fix `LuaChannel::send` so capacity-0 (rendezvous) channels always return `false` (never available to send alone)
- Send must be paired with a corresponding recv in the same atomic step
- Document the limitation for now (full rendezvous pairing is complex with flat transition model)

**Impacted models**: deadlock_circular (still works by coincidence — both sides deadlock on recv), correctness for any channel-based model

## Non-Goals

- **No new features**: No d_step, unless, remote refs, fairness, never claims, trail format, collapse compression, or CLI parity
- **No POR integration**: Bug #9 (POR not wired into main checker) is acknowledged but deferred — POR is a performance optimization, not a correctness issue
- **No ltl2ba correctness improvements**: The simplified ltl2ba is sufficient for benchmark LTL formulas; full ω-automata correctness is deferred

## Capabilities

### New Capabilities

- `local-var-prefix`: Fix codegen to prefix local variable references in guard body effects with the proctype name
- `deadlock-detection-fix`: Fix false positive deadlock detection — only flag processes with `_done == false` and zero transitions
- `array-init`: Emit array variables as Lua tables instead of scalars in state initialization
- `for-loop-codegen`: Fix for loop expansion to produce correct sequential iteration
- `ltl-checker-wiring`: Wire LTL verification (nested DFS) into the main checker pipeline
- `inline-expansion`: Implement inline macro expansion at codegen time
- `rendezvous-semantics`: Fix capacity-0 channel send to block (rendezvous mode)

### Modified Capabilities

- None (all fixes are bug fixes to existing but broken implementations)

## Impact

- `src/codegen/mod.rs`, `src/codegen/effects.rs`, `src/codegen/stmts.rs`: Local var prefixing, array init, for loop, inline expansion
- `src/runtime/mod.rs`: Deadlock detection fix, rendezvous semantics
- `src/runtime/channel.rs`: Rendezvous send fix
- `src/engine/checker/mod.rs`: LTL wiring
- `src/codegen/core.rs`: Array init in state layout
- `src/parser/mod.rs` or `src/parser/ast.rs`: Possibly adding inline expansion helpers
- Tests: updated expectations for state counts and error counts
