## Why

spin-rs can parse and verify basic Promela models, but many real-world models use mtype, typedef/struct, built-in functions (enabled, timeout, np_, len, empty, full), remote references, channel initialization syntax, and provided/priority clauses. Without these, spin-rs cannot verify models from the Spin distribution, textbooks, or industrial practice.

The original Spin grammar (spin.y) supports all these constructs. spin-rs's parser (nom-based) and AST have gaps that prevent parsing common patterns.

## What Changes

### 1. Mtype Declarations

`mtype = { red, green, blue }` defines symbolic constants. Currently missing from parser and codegen.

**Fix**: Add mtype declaration parsing. Store mtype names as integer constants (0, 1, 2, ...). Emit mtype name-to-value mapping in generated Lua for printm support.

### 2. Typedef / Struct Types

`typedef MyStruct { byte a; int b }` defines user-defined types. Currently missing entirely.

**Fix**: Add typedef parsing. Store struct field layout. Emit struct field access as nested table access in Lua (e.g., `state.var.field`). Support struct assignment and field reference.

### 3. Built-in Functions

`enabled(pid)`, `timeout`, `np_`, `len(ch)`, `empty(ch)`, `nempty(ch)`, `full(ch)`, `nfull(ch)`, `pc_value(pid)`, `eval(expr)`, `get_priority(pid)`, `set_priority(pid, val)`.

**Fix**: Add parsing for all built-in functions. Implement runtime support: enabled() checks if a process is runnable, timeout fires when no process can make progress, np_checks non-progress labels, len/empty/full query channel state, pc_value returns a process's program counter.

### 4. Remote References

`proctype[pid]@label` and `proctype[pid]:variable` allow cross-process inspection.

**Fix**: Add parsing for remote reference syntax. Implement runtime: remote label ref checks if a process is at a specific label, remote var ref reads another process's local variable.

### 5. Channel Initialization Syntax

`chan q = [N] of { byte, int }` declares a channel with typed fields.

**Fix**: Add parsing for channel init syntax. Store field types. Emit channel creation with typed slots in runtime.

### 6. Provided / Priority Clauses

`active proctype P() provided (cond)` and `active proctype P() priority N` control process scheduling.

**Fix**: Add parsing for provided and priority. Implement runtime: provided clause is checked before enabling any transition of the process, priority affects scheduling order.

## Capabilities

### New Capabilities

- `mtype`: Symbolic constant declarations work correctly
- `typedef-struct`: User-defined struct types with field access
- `builtin-functions`: enabled(), timeout, np_, len(), empty(), full(), pc_value(), eval(), get/set_priority()
- `remote-refs`: Cross-process label and variable inspection
- `channel-init`: Typed channel declarations with field types
- `provided-priority`: Process scheduling control via provided clauses and priority

### Modified Capabilities

- `promela-parser`: Full Promela grammar coverage for all standard constructs
- `codegen-core`: Generates correct Lua for struct access, mtype values, built-in functions
