# Correct Boolean Codegen

## Scope

Promela boolean operators (`&&`, `||`, `!`) produce correct 0/1 results in generated Lua code, matching Promela semantics where `0` is false and non-zero is true.

## Requirements

### REQ-1: `&&` (logical AND)

`a && b` must produce `1` if both `a` and `b` are non-zero, `0` otherwise.

Generated Lua: `((a ~= 0) and (b ~= 0)) and 1 or 0`

### REQ-2: `||` (logical OR)

`a || b` must produce `1` if either `a` or `b` is non-zero, `0` otherwise.

Generated Lua: `((a ~= 0) or (b ~= 0)) and 1 or 0`

### REQ-3: `!` (logical NOT)

`!a` must produce `1` if `a` is zero, `0` otherwise.

Generated Lua: `(a == 0) and 1 or 0`

### REQ-4: Guard evaluation

Guard conditions in `do`/`if` blocks must correctly prevent transitions when the condition is false. A state where a guarded variable is set but its guard condition is false must not be reachable.

### REQ-5: `[]p` invariant checker

The `[]p` invariant checker in `check_ltl_properties` must be re-enabled and must not produce false violations for models with correct guard evaluation.

## Verification

- `plan_5tasks_3ltls`: must report 0 errors with `[]p` fast path enabled
- All existing codegen tests must pass
- Generated Lua for `a && b`, `a || b`, `!a` must match expected patterns
