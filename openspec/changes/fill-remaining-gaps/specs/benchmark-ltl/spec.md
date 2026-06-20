## Task Reference

| Task ID | Description |
|---------|-------------|
| T4.1 | Wire `verify_ltl()` into benchmark comparison pipeline |
| T4.2 | Remove "channels deferred" skip from benchmark for deadlock_circular |
| T4.3 | Replace bare `(1)` guard truthiness with proper boolean expression |

## ADDED Requirements

### Requirement: LTL verification in benchmark

T4.1 SHALL ALWAYS use `verify_ltl()` for models that have LTL formulas. The benchmark SHALL compare spin-rs LTL results against Spin's `errors: 1` for `ltl_violation`. T4.1 SHALL detect LTL formulas in the parsed model and call the appropriate verification function.

#### Scenario: ltl_violation detected

- **WHEN** T4.1 benchmarks `ltl_violation`
- **THEN** `verify_ltl()` SHALL be called for each LTL formula
- **AND** the result SHALL be compared against Spin's `errors: 1`

### Requirement: deadlock_circular not skipped

T4.2 SHALL ALWAYS remove the `deadlock_circular` skip from the benchmark runner. The model SHALL be evaluated and compared against Spin.

### Requirement: Bare (1) guard expression

T4.3 SHALL ALWAYS ensure that bare `(1)` guards in guard conditions are treated as always-true. In the codegen, `Expression::IntLit(1)` SHALL map to `true` in Lua when used as a guard condition.
