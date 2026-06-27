## Context

The benchmark suite revealed that spin-rs explores only 1 state for most Promela models. Investigation traced the root cause to three bugs in the Lua code generator:

1. **Variable initialization dropped**: `Stmt::VarDecl` in proctype bodies emits a comment (`-- decl/label`). Variables declared locally never appear in `_spin_init_state`, so they are `nil` in Lua. Any guard referencing them (`x < 5`, `(x >= 0)`) evaluates to `false`.

2. **Break statement is a no-op**: `Stmt::Break` emits `-- break`. In Promela, `break` exits a `do/od` loop (the process stops scheduling). Without this, processes with break-based termination never stop.

3. **CLI arg parsing off by one**: `main.rs` passes `args[1..]` to `Cli::parse_from`, but `parse_from` expects the program name as first element. This causes `-a` and `--ltl` flags to be silently consumed as the program name.

Current state: All 12 benchmark models find 1 state. After fix, they should find the correct number matching Spin (10s–1000s).

## Goals / Non-Goals

**Goals:**

- Fix variable initialization in codegen so all declared variables appear in state
- Fix break statement to properly terminate process transitions
- Fix CLI arguments so `-a`, `--ltl`, etc. work correctly
- Tighten test expectations from `min_states=1` to actual correct values

**Non-Goals:**

- No changes to the verification engine, storage, search, or POR
- No changes to LTL/Büchi/NestedDFS
- No new Promela features (inline, channels beyond current, c_code)

## Decisions

**D1: Per-proctype done flag approach for break.**
Chosen: Add `state._done_<name>` boolean per proctype. Alternative considered: modifying the transitions table to exclude terminated processes. The done flag is simpler and reusable for `goto`/label-based control flow.

**D2: Walk the entire proctype body for variable declarations.**
Chosen: Recursive traversal of the AST to find all `Stmt::VarDecl` nodes. Alternative: flatten during parsing. The traversal approach is self-contained in the codegen, doesn't require AST changes.

**D3: Fix CLI by passing all args including binary name.**
Chosen: `run()` passes through all args to `Cli::parse_from()`. Alternative: use `Cli::parse()` directly in main. The through-args approach keeps the CLI module testable with custom args.

## Risks / Trade-offs

**[Risk] Variable name collisions:** A local variable `x` in proctype P and a global variable `x` would collide in the flat state table. Promela scoping rules are per-proctype.
→ **Mitigation**: Prefix local variables with `_<proctype>_` in the state table (e.g., `state.P_x`). This matches how proctype parameters are already handled.

**[Risk] Nested declarations:** Variables declared inside `if` or `do` blocks create a declaration in the outer proctype scope in Promela. The recursive traversal may incorrectly scope them.
→ **Mitigation**: After collecting all `VarDecl` nodes, deduplicate by name (first declaration wins, per Promela semantics).
