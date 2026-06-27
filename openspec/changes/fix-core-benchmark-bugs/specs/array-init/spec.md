## ADDED Requirements

### Task Reference

| Task ID | Description |
|---------|-------------|
| T3.1 | Emit arrays as Lua tables in `emit_state_layout` |
| T3.2 | Emit array access guards consistently with table storage |
| T3.3 | Verify peterson_n2 explores ~20 states after fix |

### Requirement: Array init emits Lua table

T3.1 SHALL ALWAYS emit `state.{name} = {default, default, ...}` when `VarDecl.array_size` is `Some(n)` with `n > 0`. The table SHALL have `n` elements, each initialized to `0` for numeric types or `nil` for channel types.

#### Scenario: byte flag[2]

- **WHEN** T3.1 generates `_spin_init_state` for `byte flag[2]`
- **THEN** the output SHALL contain `state.flag = {0, 0}`

#### Scenario: chan tok[5]

- **WHEN** T3.1 generates `_spin_init_state` for `chan tok[5]`
- **THEN** the output SHALL contain `state.tok = {nil, nil, nil, nil, nil}`

### Requirement: Array access consistent with table storage

T3.2 SHALL ALWAYS ensure that array accesses like `flag[_pid]` in guards and effects reference Lua table elements consistently. Promela is 0-indexed; Lua tables are 1-indexed. The existing expression codegen SHALL continue to map `flag[i]` to `s.flag[i + 1]` in Lua.

#### Scenario: flag[1 - _pid] in guard

- **WHEN** T3.2 generates code for `(flag[1 - _pid] == 0)`
- **THEN** the Lua expression SHALL be `s.flag[1 - s._pid + 1]` (or equivalent correct indexing)

### Requirement: peterson_n2 state count

T3.3 SHALL ALWAYS verify that `peterson_n2` model explores approximately 20 states after the array init fix.

#### Scenario: peterson_n2 verification

- **WHEN** T3.3 runs `verify(PETERSON_N2)` after array fix
- **THEN** `result.states_explored` SHALL be within 5% of Spin's 20
- **AND** `result.errors` SHALL BE 0
