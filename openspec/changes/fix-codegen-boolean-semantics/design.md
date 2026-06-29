## Context

The codegen translates Promela boolean operators directly to Lua equivalents:

- `&&` → `and`, `||` → `or`, `!` → `not`

Lua treats `0` as truthy, so `s.t1_1 and s.t1_2` evaluates to `true` when both are `0`. Promela requires `0 && 0` to be `0` (false). This causes guards to fire when conditions are false, producing unreachable states that violate `[]p` invariants.

## Goals / Non-Goals

**Goals:**

- Promela `&&`, `||`, `!` produce correct 0/1 results in generated Lua
- All existing tests continue to pass
- `[]p` invariant checker can be re-enabled as a fast path

**Non-Goals:**

- Full Promela boolean semantics for non-integer types (bit, bool are stored as integers)
- Performance optimization of the normalization

## Decisions

**Decision 1: Normalize at codegen time, not runtime**

Each boolean sub-expression is wrapped with `~= 0` to convert to Lua boolean, then `and 1 or 0` converts back to 0/1:

```
Promela:  a && b
Lua:      ((a ~= 0) and (b ~= 0)) and 1 or 0
```

This is done in `binary_op_to_lua` and `unary_to_lua` — a localized change that doesn't require modifying the expression tree or adding a normalization pass.

**Decision 2: Always produce 0/1, not Lua booleans**

The `and 1 or 0` suffix ensures the result is always an integer (0 or 1), matching Promela semantics where boolean expressions produce integer results. This is important for contexts like `x = a && b` where the result is assigned to an integer variable.

**Decision 3: Re-enable `[]p` invariant checker**

Once guard evaluation is correct, the `[]p` invariant checker can be safely re-enabled. It's a single DFS pass that checks each state against the condition — much faster than full nested DFS for simple `[]p` formulas.

## Risks / Trade-offs

- **Code size**: Generated Lua will be slightly larger (each boolean op gets `~= 0` wrappers)
- **Performance**: The `~= 0` check is cheap (integer comparison), negligible overhead
- **Edge case**: `not` on non-boolean expressions (e.g., `!x` where `x` is an integer counter) — Promela semantics say `!x` is `1` if `x == 0`, `0` otherwise. The fix `(x == 0) and 1 or 0` matches this correctly.
