## Context

The parser in `src/parser/` uses nom combinators. The AST in `src/parser/ast.rs` defines the IR. The codegen in `src/codegen/` translates AST to Lua. Each new language feature requires changes in all three layers.

## Goals / Non-Goals

**Goals:**

- Parse all Promela constructs that Spin supports
- Generate correct Lua for each construct
- All existing tests continue to pass
- Real-world Promela models from Spin distribution parse and verify

**Non-Goals:**

- Full Spin-compatible error messages
- Support for c_code/c_decl/c_expr (separate concern)
- Support for event traces (separate concern)

## Decisions

**Decision 1: Mtype as integer constants**

Mtype names are stored as integers (0, 1, 2, ...) in the state vector, matching Spin's internal representation. A separate mapping table maps names to values for printm support. The parser assigns sequential IDs.

**Decision 2: Struct fields as nested Lua tables**

A struct variable `s` with fields `a` and `b` becomes `state.s = { a = 0, b = 0 }` in Lua. Field access `s.a` becomes `state.s.a`. Struct assignment copies the entire table. This is simple and matches Lua's table semantics.

**Decision 3: Built-in functions as Lua FFI**

Each built-in function becomes a Lua FFI call:

- `enabled(pid)` → `_spin_enabled(pid)` — checks if process pid is runnable
- `timeout` → `_spin_timeout()` — returns true if no process can make progress
- `np_` → `_spin_np_()` — returns true if current state has no progress label
- `len(ch)` → `chan_len(ch)` — already exists
- `empty(ch)` → `chan_empty(ch)` — already exists
- `full(ch)` → `chan_full(ch)` — already exists
- `pc_value(pid)` → `_spin_pc_value(pid)` — returns step number of process pid

**Decision 4: Remote references via runtime lookup**

Remote label ref `P[pid]@label` checks if process `pid`'s step variable equals the label's step number. Remote var ref `P[pid]:var` reads `state.P_pid_var` from the state vector. Both are implemented as Lua FFI calls.

**Decision 5: Channel init with typed fields**

`chan q = [5] of { byte, int }` creates a channel with 5 slots, each slot holding 2 fields (byte + int). The runtime stores field types and validates send/receive field counts and types.

**Decision 6: Provided clause as global guard**

A provided clause on a proctype is AND-ed with every transition guard of that proctype. If the provided condition is false, no transition of that process is enabled. Priority is stored in the state vector and checked during scheduling.

## Risks / Trade-offs

- **Struct complexity**: Nested structs and struct arrays increase state vector size. Each struct field is a separate Lua table entry
- **Remote ref performance**: Cross-process inspection requires accessing another process's state, which is O(1) with the step-variable approach
- **Provided clause overhead**: AND-ing provided with every guard increases guard evaluation cost slightly
