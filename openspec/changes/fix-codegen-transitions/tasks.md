## 1. Variable Initialization

- [x] 1.1 Add recursive AST traversal to collect all `Stmt::VarDecl` from proctype bodies (including nested in if/do/atomic/d_step)
- [x] 1.2 Emit collected variables in `_spin_init_state` with default values via `default_value()`
- [x] 1.3 Handle `init` field in `VarDecl` — emit user-specified initializer instead of default when present
- [x] 1.4 Update `emit_state_layout` to call the new collection logic for all proctypes
- [x] 1.5 Add test verifying state blob contains all declared variables with correct defaults
- [x] 1.6 Add test for `do :: (1) -> skip od` pattern — must explore at least 2 states

## 2. Break Handling

- [x] 2.1 Add per-proctype `_done_<name>` flag to state vector, initialized to `false`
- [x] 2.2 Update `emit_proctype` to check `_done_<name>` at the top and return `{}` if set
- [x] 2.3 Update `emit_guards` to set `_done_<name> = true` in the effect when a `break` is encountered
- [x] 2.4 Add test for loop with break: `do :: x < 5 -> x = x + 1 :: x >= 5 -> break od` — at least 6 states

## 3. CLI Fix

- [x] 3.1 Fix `main.rs` to pass all args (including binary name) to `run()`
- [x] 3.2 Verify `spin-rs -a model.pml` prints Lua source
- [x] 3.3 Verify `spin-rs --ltl name formula model.pml` works

## 4. Validation

- [x] 4.1 Run benchmark suite and confirm state counts match Spin expected ranges
- [x] 4.2 Tighten integration test state count bounds from `min=1` to actual expected values
- [x] 4.3 Verify `multi_process` model still works (regression check for global+local vars)
- [x] 4.4 Verify `assertion_safety` model (simple assignment — still works)
