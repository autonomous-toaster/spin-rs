## 1. Fix Boolean Operator Codegen

- [ ] 1.1 Fix `binary_op_to_lua` for `And`: emit `((left ~= 0) and (right ~= 0)) and 1 or 0`
- [ ] 1.2 Fix `binary_op_to_lua` for `Or`: emit `((left ~= 0) or (right ~= 0)) and 1 or 0`
- [ ] 1.3 Fix `unary_to_lua` for `Not`: emit `(expr == 0) and 1 or 0`
- [ ] 1.4 Run all codegen tests and confirm generated Lua matches expected patterns

## 2. Re-enable `[]p` Invariant Checker

- [ ] 2.1 Re-enable `check_ltl_properties` in `check_dfs` to use `[]p` invariant checker for simple formulas
- [ ] 2.2 Run benchmark and confirm `plan_5tasks_3ltls` reports 0 errors with `[]p` fast path
- [ ] 2.3 Run `cargo test --workspace` and confirm no regressions
- [ ] 2.4 Run `cargo clippy --workspace --all-targets` and confirm clean
