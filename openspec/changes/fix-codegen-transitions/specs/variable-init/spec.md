## Task Reference

| Task ID | Description |
|---------|-------------|
| T1.1 | Collect all variable declarations from proctype bodies and parameters |
| T1.2 | Emit local variable initializers in `_spin_init_state` with default values |
| T1.3 | Ensure `do :: (1) -> ... od` pattern works (guards are always-true) |
| T1.4 | Add test verifying state vector includes all declared variables |

## ADDED Requirements

### Requirement: Collect local variable declarations

T1.1 SHALL ALWAYS traverse each proctype's body AST and collect all `Stmt::VarDecl` nodes. T1.1 SHALL handle both top-level body declarations and declarations inside control flow blocks. T1.1 SHALL deduplicate variable names.

#### Scenario: Single proctype with local variable

- **WHEN** T1.1 processes `active proctype P() { byte x; x = 1; }`
- **THEN** T1.1 SHALL find `x` as a local variable declaration

#### Scenario: Multiple proctypes with different locals

- **WHEN** T1.1 processes `active proctype P() { byte x; } active proctype Q() { byte y; }`
- **THEN** T1.1 SHALL find `x` in proctype P and `y` in proctype Q

### Requirement: Emit variable initializers in init state

T1.2 SHALL ALWAYS emit Lua code in `_spin_init_state` that initializes each local variable to its default value. T1.2 SHALL use the existing `default_value()` function.

#### Scenario: Default values

- **WHEN** T1.2 generates Lua for `active proctype P() { byte x; bool flag; }`
- **THEN** `_spin_init_state` SHALL set `state.x = 0` and `state.flag = false`

#### Scenario: With initializer expression

- **WHEN** T1.2 generates Lua for `active proctype P() { byte x = 5; }`
- **THEN** `_spin_init_state` SHALL set `state.x = 5`

### Requirement: Always-true guard pattern

T1.3 SHALL ALWAYS ensure guards like `(1)` are treated as always-true. T1.3 SHALL test that `active proctype P() { do :: (1) -> skip od }` does not get stuck at 1 state.

#### Scenario: Bare (1) guard

- **WHEN** T1.3 verifies `active proctype P() { do :: (1) -> skip od }`
- **THEN** the verifier SHALL explore at least 2 states (initial + one iteration)

### Requirement: State vector test

T1.4 SHALL ALWAYS add a test that verifies the serialized state blob includes all declared variables. T1.4 SHALL verify `LuaModel::init_states()` returns a blob containing the variable name and its default value.

#### Scenario: State blob contains variables

- **WHEN** T1.4 creates a `LuaModel` from `active proctype P() { byte x; }`
- **THEN** the init state blob SHALL contain `"x"` with value `0`
