## Task Reference

| Task ID | Description |
|---------|-------------|
| T1.1 | Parse `chan ch = [N] of { byte }` into `TopLevel::ChanDecl` |
| T1.2 | Emit channel name in `_spin_init_state` state blob |
| T1.3 | Wire ChanDecl in runtime `from_model` (currently dead code) |
| T1.4 | Test: `deadlock_circular` parses without error |

## ADDED Requirements

### Requirement: Parse channel declarations

T1.1 SHALL ALWAYS parse `chan <name> = [<capacity>] of { <type> }` into `TopLevel::ChanDecl { name, capacity, line }`. The parser SHALL recognize `chan` as a top-level declaration type, NOT as a `var_decl`. The existing `var_decl`-based parsing of channels SHALL BE replaced.

#### Scenario: Basic channel declaration

- **WHEN** T1.1 parses `chan ch = [0] of { byte };`
- **THEN** a `TopLevel::ChanDecl` SHALL BE created with `name = "ch"` and `capacity = 0`

#### Scenario: Multi-channel declarations

- **WHEN** T1.1 parses `chan ch1 = [0] of { byte }; chan ch2 = [0] of { byte };`
- **THEN** two `ChanDecl` nodes SHALL BE created

### Requirement: Channel state in init blob

T1.2 SHALL ALWAYS emit channel state in `_spin_init_state`. Each channel SHALL appear in the state table with its name and a placeholder value (e.g., `nil` or `"channel:ch"`). This ensures the channel is part of the state vector for transition enumeration.

#### Scenario: Channel in init state

- **WHEN** T1.2 generates Lua for `chan ch = [2] of { byte };`
- **THEN** `_spin_init_state` SHALL set `state.ch = nil` or equivalent

### Requirement: Runtime wires ChanDecl

T1.3 SHALL ALWAYS check for both `TopLevel::ChanDecl` AND `TopLevel::GlobalVar(VarDecl { var_type: Chan })` in `from_model`. The existing dead branch checking for `ChanDecl` SHALL be updated to also match the parser's output. Alternatively, T1.3 SHALL update `from_model` to match the new parser output after T1.1.

#### Scenario: Channel registered

- **WHEN** T1.3 processes `chan ch = [0] of { byte };`
- **THEN** the runtime SHALL register channel `ch` with capacity `0`

### Requirement: Parsing smoke test

T1.4 SHALL ALWAYS add a test that verifies the full `deadlock_circular` model parses correctly.

#### Scenario: deadlock_circular parses

- **WHEN** T1.4 attempts to parse `DEADLOCK_CIRCULAR`
- **THEN** the parse SHALL succeed
- **AND** SHALL produce at least 2 `ChanDecl` and 2 `ProctypeDecl` nodes
