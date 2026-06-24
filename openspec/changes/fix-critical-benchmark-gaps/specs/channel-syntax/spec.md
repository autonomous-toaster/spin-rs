## Task Reference

| Task ID | Description |
|---------|-------------|
| T1.1 | Add parser rule for `chan name = [N] of { type };` syntax |
| T1.2 | Extract capacity and message type from channel declarations |
| T1.3 | Generate channel state in `_spin_init_state` |
| T1.4 | Wire `ChanDecl` in runtime `from_model()` |
| T1.5 | Test: `deadlock_circular` parses correctly |
| T1.6 | Test: `deadlock_circular` finds exactly 1 error (deadlock) |

## ADDED Requirements

### Requirement: Parse channel syntax

T1.1 SHALL add a parser rule for `chan name = [N] of { type };` syntax. The parser SHALL handle:

- Channel name (identifier)
- Capacity (integer in brackets)
- Message type (basic types: byte, int, bool, chan)

The parser SHALL produce `TopLevel::ChanDecl { name, capacity, line }`.

#### Scenario: Channel with capacity 0 (rendezvous)

- **WHEN** T1.1 parses `chan ch1 = [0] of { byte };`
- **THEN** it SHALL produce `ChanDecl { name: "ch1", capacity: 0 }`

#### Scenario: Channel with non-zero capacity

- **WHEN** T1.1 parses `chan ch2 = [5] of { int };`
- **THEN** it SHALL produce `ChanDecl { name: "ch2", capacity: 5 }`

### Requirement: Generate channel state

T1.3 SHALL emit channel initialization in `_spin_init_state`. For each `ChanDecl`, the codegen SHALL emit:

```lua
state.ch_name = nil  -- or appropriate initial state
```

#### Scenario: Channel state in init

- **WHEN** T1.3 generates Lua for a model with `chan ch = [0] of { byte };`
- **THEN** `_spin_init_state` SHALL include `state.ch = nil`

### Requirement: Runtime channel wiring

T1.4 SHALL wire `ChanDecl` in `LuaModel::from_model()`. For each `ChanDecl`, the runtime SHALL:

- Register the channel with the correct capacity
- Make it available for send/recv operations

#### Scenario: Channel registration

- **WHEN** T1.4 processes a `ChanDecl { name: "ch", capacity: 5 }`
- **THEN** it SHALL register channel "ch" with capacity 5

### Requirement: Deadlock detection test

T1.6 SHALL verify that `deadlock_circular` model detects exactly 1 error. The model has:

- Two processes P and Q
- P tries to send on ch1, then receive on ch2
- Q tries to send on ch2, then receive on ch1
- This creates a circular deadlock

#### Scenario: Deadlock detected

- **WHEN** T1.6 runs `verify(DEADLOCK_CIRCULAR)`
- **THEN** the result SHALL have `errors == 1`
- **AND** the violation description SHALL contain "deadlock"
