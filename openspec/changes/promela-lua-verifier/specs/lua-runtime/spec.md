## Task Reference

| Task ID | Description |
|---------|-------------|
| T3.1 | Bootstrap mlua runtime and expose Rust state representation |
| T3.2 | Execute generated Lua transition code within the runtime |
| T3.3 | Bridge state comparison and hashing between Rust and Lua |

## ADDED Requirements

### Requirement: Bootstrap Lua runtime

T3.1 SHALL initialize an mlua instance with loaded standard libraries and a prelude defining Promela runtime primitives. T3.1 SHALL expose the Rust state vector to Lua as a userdata or table with indexed access. T3.1 SHALL complete BEFORE T3.2 SHALL execute generated code.

#### Scenario: Runtime initialization

- **WHEN** T3.1 initializes mlua for model `model.lua`
- **THEN** T3.1 SHALL load the generated Lua and make the Rust state vector readable/writable from Lua

#### Scenario: Channel primitives available

- **WHEN** T3.1 loads the prelude
- **THEN** T3.1 SHALL register `channel_send`, `channel_recv`, `channel_len`, `channel_full`, `channel_empty` as callable Lua functions backed by Rust

### Requirement: Execute generated transitions

T3.2 SHALL call generated Lua transition functions from Rust, passing the current state and receiving a list of enabled (guard, effect) pairs. T3.2 SHALL execute the effect function to produce a new state. T3.2 SHALL complete AFTER T3.1 SHALL bootstrap the runtime.

#### Scenario: Single transition evaluation

- **WHEN** T3.2 calls a generated proctype's transition function with state `{x = 0}`
- **THEN** T3.2 SHALL return a list where each enabled entry has a guard predicate and an effect function

#### Scenario: Guard disables transition

- **WHEN** T3.2 calls a transition whose guard checks `state.x > 0` but `state.x == 0`
- **THEN** T3.2 SHALL not include that transition in the returned list

### Requirement: Bridge state hashing

T3.3 SHALL expose the Rust hash function to Lua so that generated code can compute deterministic fingerprints of the state vector. T3.3 SHALL ensure that hashing and comparison operate on the Rust side for performance. T3.3 SHALL complete AFTER T3.1 SHALL bootstrap.

#### Scenario: State fingerprint

- **WHEN** T3.3 computes a hash of state `{x = 1, y = 2}`
- **THEN** T3.3 SHALL produce the same u64 fingerprint as a Rust-side hash of the equivalent byte vector

#### Scenario: State equality

- **WHEN** T3.3 compares two identical states
- **THEN** T3.3 SHALL return true; two different states SHALL return false
