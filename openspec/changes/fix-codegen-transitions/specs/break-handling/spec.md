## Task Reference

| Task ID | Description |
|---------|-------------|
| T2.1 | Add a per-proctype `_done` flag tracked in state |
| T2.2 | Emit break effect to set `_done` flag |
| T2.3 | Update `_spin_transitions_P` to check `_done` before enumerating |
| T2.4 | Add test verifying break terminates process transitions |

## ADDED Requirements

### Requirement: Per-proctype done flag

T2.1 SHALL ALWAYS add a `_done_<proctype_name>` boolean field to the state vector for each proctype. T2.1 SHALL initialize the flag to `false` in `_spin_init_state`.

#### Scenario: Init state has done flags

- **WHEN** T2.1 processes `active proctype P() { byte x; do :: (1) -> break od }`
- **THEN** `_spin_init_state` SHALL set `state._done_P = false`

### Requirement: Break sets done flag

T2.2 SHALL ALWAYS emit `state._done_<proctype_name> = true` as the effect of a `break` statement in a `do/od` guard body.

#### Scenario: Break effect

- **WHEN** T2.2 emits code for `do :: (x >= 5) -> break od`
- **THEN** the effect function SHALL set the done flag to true

### Requirement: Guard done flag

T2.3 SHALL ALWAYS update `_spin_transitions_P` to check the done flag. When `state._done_P` is true, the function SHALL return an empty table.

#### Scenario: Done flag prevents transitions

- **WHEN** T2.3 checks `state._done_P` at the top of `_spin_transitions_P`
- **THEN** the function SHALL return `{}` if the flag is true

### Requirement: Break termination test

T2.4 SHALL ALWAYS add a test verifying that a model with a break in a do/od loop eventually stops exploring.

#### Scenario: Loop with break terminates

- **WHEN** T2.4 verifies `active proctype P() { byte x; do :: x < 5 -> x = x + 1 :: x >= 5 -> break od }`
- **THEN** the verifier SHALL explore at least 6 states (x=0 through x=5)
- **AND** the process SHALL stop scheduling after break
