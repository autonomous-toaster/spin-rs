## Phase 1: Channel Syntax Parser (BLOCKING)

- [ ] 1.1 Add parser rule for `chan name = [N] of { type };` syntax
  - Parse channel name (identifier)
  - Parse capacity (integer in brackets)
  - Parse message type (basic types: byte, int, bool, chan)
  - Produce `TopLevel::ChanDecl { name, capacity, line }`
- [ ] 1.2 Add `ChanDecl` to `top_level()` parser alternatives
- [ ] 1.3 Generate channel state in `_spin_init_state` for each `ChanDecl`
- [ ] 1.4 Wire `ChanDecl` in runtime `LuaModel::from_model()`
  - Register channels with correct capacity
  - Make available for send/recv operations
- [ ] 1.5 Test: `deadlock_circular` parses correctly (2 processes, 2 channels)
- [ ] 1.6 Test: `deadlock_circular` finds exactly 1 error (deadlock)
- [ ] 1.7 Remove "channels deferred" skip from benchmark for `token_ring_n5`

## Phase 2: LTL Integration (BLOCKING)

- [ ] 2.1 Detect LTL formulas in parsed models (`TopLevel::Ltl`)
- [ ] 2.2 Wire `verify_ltl()` into benchmark for models with LTL
- [ ] 2.3 Extract LTL formula string and name from `LtlFormula`
- [ ] 2.4 Compare spin-rs LTL results against Spin's error counts
- [ ] 2.5 Test: `ltl_violation` detects exactly 1 error

## Phase 3: Boolean Expression Scoping (REGRESSION FIX)

- [ ] 3.1 Add `in_guard_context` parameter to `expr_to_lua()` methods
- [ ] 3.2 Apply `~= 0` check ONLY when `in_guard_context=true` AND variable is boolean
- [ ] 3.3 Track `bool_vars` HashSet in `LuaGenerator` (from global var declarations)
- [ ] 3.4 Update `emit_guards()` to call `expr_to_lua(e, true)` for guard conditions
- [ ] 3.5 Update `emit_assignment()` to call `expr_to_lua(e, false)` for RHS expressions
- [ ] 3.6 Test: `single_loop` returns to 102+ states (was 102, regressed to 2)
- [ ] 3.7 Validate all models: no new regressions introduced

## Phase 4: Benchmark Validation

- [ ] 4.1 Run full benchmark suite
- [ ] 4.2 Validate `deadlock_circular`: 1 error (deadlock detected)
- [ ] 4.3 Validate `ltl_violation`: 1 error (LTL violation detected)
- [ ] 4.4 Validate `single_loop`: 102+ states (regression fixed)
- [ ] 4.5 Document remaining gaps (if any)
