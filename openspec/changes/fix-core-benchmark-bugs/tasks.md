## 1. Fix Local Variable Prefix in Codegen

- [x] 1.1 Fix `emit_guards` inline assignment effect to prefix targets with `current_proctype`
- [x] 1.2 Fix `emit_assignment_effect` to prefix targets with `current_proctype`
- [x] 1.3 Run `debug_single_loop` and verify generated Lua uses `P_x` not `x` in effects
- [x] 1.4 Run benchmark and confirm single_loop state count improves from 2 to ~103

## 2. Fix Deadlock False Positive

- [x] 2.1 Parse per-process `_done_` flags from state blob instead of freetext search
- [x] 2.2 Only flag deadlock when at least one non-done process has zero transitions
- [x] 2.3 Run benchmark and confirm plan_5tasks_3ltls and multi_process report 0 errors
- [x] 2.4 Confirm deadlock_circular still reports 1 error

## 3. Fix Array Initialization

- [x] 3.1 Emit arrays as Lua tables in `emit_state_layout` when `array_size > 0`
- [x] 3.2 Verify generated Lua for `byte flag[2]` produces `state.flag = {0, 0}`
- [x] 3.3 Run benchmark and confirm peterson_n2 state count improves from 1 to ~20

## 4. Fix For Loop Codegen

- [ ] 4.1 Expand `for` loops at parse time into sequential statements (iterate start..end)
- [ ] 4.2 Verify token_ring_n5 init block correctly expands for (i in 0 .. 4)
- [ ] 4.3 Run benchmark and confirm token_ring_n5 still passes (regression check)

## 5. Wire LTL Verification into Checker

- [ ] 5.1 Call `PropertyChecker::check_liveness` in `Checker::check_dfs` when model has LTL formulas
- [ ] 5.2 Report LTL violations in `result.violations` and increment `result.errors`
- [ ] 5.3 Run benchmark and confirm ltl_violation reports 1 error

## 6. Implement Inline Expansion

- [ ] 6.1 Collect inline definitions into HashMap during codegen init
- [ ] 6.2 Expand inline calls by substituting parameters and emitting body
- [ ] 6.3 Verify dining_n4 model parses and explores more than 1 state

## 7. Fix Rendezvous Channel Semantics

- [ ] 7.1 Modify `LuaChannel::send` to return `false` for capacity-0 channels
- [ ] 7.2 Verify deadlock_circular still detects 1 error (fix doesn't break existing)
- [ ] 7.3 Document rendezvous limitation in design.md

## 8. Final Verification

- [ ] 8.1 Run full benchmark suite and record pass/fail counts per model
- [ ] 8.2 Run `cargo test --workspace` and confirm no regressions
- [ ] 8.3 Run `cargo clippy --workspace --all-targets -- -Dwarnings` and confirm clean
