## Why

Lua treats `0` as truthy, but Promela booleans are stored as integers (`0` = false, non-zero = true). The codegen translates Promela `&&`, `||`, and `!` directly to Lua `and`, `or`, `not` without normalizing operands, causing guards to evaluate incorrectly — transitions fire when their conditions are false. This was the root cause of the false `[]p` violations found during `fix-core-benchmark-bugs`.

## What Changes

- Fix `binary_op_to_lua` in `src/codegen/expr_utils.rs` to normalize operands for `And` and `Or`: `((left ~= 0) and (right ~= 0)) and 1 or 0`
- Fix `unary_to_lua` for `Not`: `(expr == 0) and 1 or 0` instead of `not expr`
- Re-enable the `[]p` invariant checker in `check_ltl_properties` now that guard evaluation is correct
- Verify `plan_5tasks_3ltls` reports 0 errors with the `[]p` fast path (not just nested DFS)

## Capabilities

### New Capabilities

- `correct-boolean-codegen`: Promela boolean operators produce correct 0/1 results in generated Lua

### Modified Capabilities

- `ltl-verification`: `[]p` invariant checker can be safely re-enabled as a fast path
