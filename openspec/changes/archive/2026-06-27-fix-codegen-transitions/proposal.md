## Why

The benchmark suite revealed that spin-rs explores only 1 state for most Promela models. The root cause is that local variable declarations inside proctypes are dropped during Lua code generation — `Stmt::VarDecl` emits a comment. This means variables like `byte x` are `nil` in Lua, guards like `x < 5` evaluate to `false`, and no transitions ever fire.

Without fixing this, spin-rs is effectively non-functional for any model with local variables — which is almost all of them.

## What Changes

- Fix codegen to emit local variable initialization in `_spin_init_state` for all variable declarations (global, proctype parameters, and local body declarations)
- Fix codegen to handle `break` statements in `do/od` loops (currently emitted as comments)
- Fix CLI argument parsing (off-by-one due to `Cli::parse_from` call)
- Verify correctness by running the benchmark suite and ensuring state counts match Spin within tolerance
- Fix the `do :: (1) -> ... od` always-true guard pattern used by veriplan-generated models

**Non-goals:**

- No changes to the verification engine itself (storage, search, POR)
- No changes to LTL/Büchi/NestedDFS

## Capabilities

### New Capabilities

- `variable-init`: Emit local variable declarations as state initialization, with correct default values per type
- `break-handling`: Implement `break` in `do/od` guard effects (terminate process eligibility)
- `cli-parsing`: Fix the off-by-one argument parsing bug in the CLI entry point

### Modified Capabilities

None.

## Impact

- `src/codegen/mod.rs`: Major changes to `emit_state_layout`, `emit_stmts`, and `emit_guards`
- `src/main.rs` and/or `src/cli/mod.rs`: Fix CLI argument passing
- Test suite: State count expectations tighten from `min=1` to actual expected values
- Benchmark: State counts should jump from 1 to expected values (10s–1000s)
