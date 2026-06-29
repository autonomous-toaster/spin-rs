## Phase 1: Mtype

- [ ] 1.1 Add mtype declaration parsing: `mtype = { name1, name2, ... }`
- [ ] 1.2 Assign sequential integer IDs to mtype names
- [ ] 1.3 Store mtype name-to-value mapping in AST
- [ ] 1.4 Emit mtype mapping table in generated Lua
- [ ] 1.5 Support mtype in variable declarations: `mtype x`
- [ ] 1.6 Support mtype comparison: `x == red`
- [ ] 1.7 Add test: mtype declaration and comparison
- [ ] 1.8 Add test: mtype in channel send/receive

## Phase 2: Typedef / Struct

- [ ] 2.1 Add typedef parsing: `typedef MyStruct { byte a; int b }`
- [ ] 2.2 Add struct variable declaration parsing: `MyStruct s`
- [ ] 2.3 Add struct field access parsing: `s.a`, `s.b`
- [ ] 2.4 Add struct assignment parsing: `s = t`
- [ ] 2.5 Emit struct fields as nested Lua tables
- [ ] 2.6 Emit struct field access as table field access
- [ ] 2.7 Emit struct assignment as table copy
- [ ] 2.8 Add test: struct declaration and field access
- [ ] 2.9 Add test: struct assignment
- [ ] 2.10 Add test: struct array

## Phase 3: Built-in Functions

- [ ] 3.1 Add parsing for `enabled(expr)`
- [ ] 3.2 Add parsing for `timeout`
- [ ] 3.3 Add parsing for `np_`
- [ ] 3.4 Add parsing for `len(expr)`, `empty(expr)`, `full(expr)`, `nempty(expr)`, `nfull(expr)`
- [ ] 3.5 Add parsing for `pc_value(expr)`
- [ ] 3.6 Add parsing for `eval(expr)`
- [ ] 3.7 Add parsing for `get_priority(expr)`, `set_priority(expr, expr)`
- [ ] 3.8 Implement `_spin_enabled` FFI: check if process pid is runnable
- [ ] 3.9 Implement `_spin_timeout` FFI: true when no process can make progress
- [ ] 3.10 Implement `_spin_np_` FFI: true when current state has no progress label
- [ ] 3.11 Implement `_spin_pc_value` FFI: return step number of process pid
- [ ] 3.12 Implement `_spin_get_priority` / `_spin_set_priority` FFI
- [ ] 3.13 Update codegen to emit FFI calls for built-in functions
- [ ] 3.14 Add test: enabled() in guard
- [ ] 3.15 Add test: timeout in never claim
- [ ] 3.16 Add test: len()/empty()/full() channel queries

## Phase 4: Remote References

- [ ] 4.1 Add parsing for `PNAME[expr]@NAME` (remote label ref)
- [ ] 4.2 Add parsing for `PNAME[expr]:NAME` (remote var ref)
- [ ] 4.3 Add parsing for `PNAME@NAME` (unindexed remote label ref)
- [ ] 4.4 Add parsing for `PNAME:NAME` (unindexed remote var ref)
- [ ] 4.5 Implement `_spin_remote_label` FFI: check if process is at label
- [ ] 4.6 Implement `_spin_remote_var` FFI: read another process's variable
- [ ] 4.7 Update codegen to emit FFI calls for remote refs
- [ ] 4.8 Add test: remote label reference in guard
- [ ] 4.9 Add test: remote variable read

## Phase 5: Channel Initialization

- [ ] 5.1 Add parsing for `chan name = [N] of { type1, type2, ... }`
- [ ] 5.2 Store field types in channel metadata
- [ ] 5.3 Emit channel creation with typed slots in runtime
- [ ] 5.4 Validate send/receive field counts against channel type
- [ ] 5.5 Add test: typed channel send/receive
- [ ] 5.6 Add test: field count mismatch error

## Phase 6: Provided / Priority

- [ ] 6.1 Add parsing for `provided (expr)` on proctype
- [ ] 6.2 Add parsing for `priority N` on proctype
- [ ] 6.3 AND provided clause with every transition guard in codegen
- [ ] 6.4 Store priority in state vector
- [ ] 6.5 Implement priority-based scheduling in engine
- [ ] 6.6 Add test: provided clause prevents process execution
- [ ] 6.7 Add test: priority affects scheduling order
