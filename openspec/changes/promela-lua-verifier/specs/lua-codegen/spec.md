## Task Reference

| Task ID | Description |
|---------|-------------|
| T2.1 | Translate Promela IR to Lua source code |
| T2.2 | Generate state vector layout as Lua table template |
| T2.3 | Generate per-proctype transition functions as Lua closures |

## ADDED Requirements

### Requirement: Compile Promela IR to Lua

T2.1 SHALL translate the Promela IR (AST after semantic analysis) into equivalent Lua source code. Each Promela proctype SHALL become a Lua function. T2.1 SHALL complete BEFORE T2.2 SHALL generate the state layout.

#### Scenario: Proctype to Lua function

- **WHEN** T2.1 receives `active proctype P() { byte x; x = 1; }`
- **THEN** T2.1 SHALL produce a Lua function that sets `state.x = 1` when its guard is true

#### Scenario: Guarded command (if/fi)

- **WHEN** T2.1 receives `if :: (x > 0) -> y = 1 :: else -> y = 0 fi`
- **THEN** T2.1 SHALL produce Lua code equivalent to `if state.x > 0 then state.y = 1 else state.y = 0 end`

#### Scenario: Loop (do/od) with break

- **WHEN** T2.1 receives `do :: (x > 0) -> x = x - 1 :: (x == 0) -> break od`
- **THEN** T2.1 SHALL produce a Lua loop with conditional exit

### Requirement: Generate state vector layout

T2.2 SHALL generate Lua code that declares the state vector as a Lua table or userdata, mapping Promela variables to fields. T2.2 SHALL complete BEFORE T2.3 SHALL generate transition functions.

#### Scenario: Basic variables

- **WHEN** T2.2 receives `byte x, y; bit flag;`
- **THEN** T2.2 SHALL produce `state = { x = 0, y = 0, flag = 0 }`

#### Scenario: Channel variable

- **WHEN** T2.2 receives `chan ch = [2] of { byte }`
- **THEN** T2.2 SHALL produce a Lua table with a `ch` field containing a channel descriptor (buffer, capacity, field types)

### Requirement: Generate per-proctype transitions

T2.3 SHALL generate Lua closures for each proctype that enumerate enabled transitions given a state. Each closure SHALL return a list of (guard, effect) pairs where guard is a predicate on the state and effect is a function mutating the state. T2.3 SHALL complete AFTER T2.2 SHALL define the state layout.

#### Scenario: Single transition

- **WHEN** T2.3 receives `active proctype P() { x = x + 1 }`
- **THEN** T2.3 SHALL produce a closure returning `{ { guard = function() return true end, effect = function(s) s.x = s.x + 1 end } }`

#### Scenario: Blocking receive

- **WHEN** T2.3 receives `active proctype P() { ch?msg }`
- **THEN** T2.3 SHALL produce a closure whose guard checks `ch` is non-empty and whose effect dequeues the first message
