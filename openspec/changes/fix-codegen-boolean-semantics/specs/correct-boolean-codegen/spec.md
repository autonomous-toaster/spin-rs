# Correct Boolean Codegen

## Scope

Promela boolean operators (`&&`, `||`, `!`) produce correct 0/1 results in generated Lua code, matching Promela semantics where `0` is false and non-zero is true.

## Requirements

### Requirement: AND operator

T1.1 SHALL ALWAYS produce `1` for `a && b` when both `a` and `b` are non-zero, and `0` otherwise.

Generated Lua: `((a ~= 0) and (b ~= 0)) and 1 or 0`

### Requirement: OR operator

T1.2 SHALL ALWAYS produce `1` for `a || b` when either `a` or `b` is non-zero, and `0` otherwise.

Generated Lua: `((a ~= 0) or (b ~= 0)) and 1 or 0`

### Requirement: NOT operator

T1.3 SHALL ALWAYS produce `1` for `!a` when `a` is zero, and `0` otherwise.

Generated Lua: `(a == 0) and 1 or 0`

### Requirement: Guard evaluation

T1.4 SHALL ALWAYS prevent transitions from firing when their guard condition is false. A state where a guarded variable is set but its guard condition is false SHALL NOT be reachable.

### Requirement: Invariant checker re-enable

T2.1 SHALL ALWAYS re-enable the `[]p` invariant checker in `check_ltl_properties`.

### Requirement: Invariant checker correctness

T2.1 SHALL ALWAYS NOT produce false violations for models with correct guard evaluation.

## Verification

- `plan_5tasks_3ltls`: must report 0 errors with `[]p` fast path enabled
- All existing codegen tests must pass
- Generated Lua for `a && b`, `a || b`, `!a` must match expected patterns
