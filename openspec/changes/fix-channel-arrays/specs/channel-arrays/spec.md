## ADDED Requirements

### Requirement: Parse channel array declarations

The parser SHALL recognize `chan <name>[<size>];` syntax as a channel array declaration. The declaration creates an array of `<size>` rendezvous channels (capacity 0). Each channel in the array SHALL be individually addressable via indexed access.

#### Scenario: Basic channel array declaration

- **WHEN** the parser encounters `chan tok[5];`
- **THEN** it SHALL produce a `TopLevel::ChannelArray` AST node with `name = "tok"` and `size = 5`
- **AND** the array SHALL represent 5 separate rendezvous channels

#### Scenario: Channel array with different sizes

- **WHEN** the parser encounters `chan small[2];` and `chan large[10];`
- **THEN** it SHALL produce two `ChannelArray` nodes with sizes 2 and 10 respectively
- **AND** each array SHALL create the correct number of channels

#### Scenario: Channel array declaration syntax variations

- **WHEN** the parser encounters `chan ch[1];` (single channel array)
- **THEN** it SHALL parse successfully as a `ChannelArray` with `size = 1`

### Requirement: Parse indexed channel access in send statements

The parser SHALL accept expressions (not just identifiers) as the channel target in send statements. The syntax `chan[expr] ! args` SHALL be parsed as a send operation to the channel at index `expr` in the array `chan`.

#### Scenario: Send with literal index

- **WHEN** the parser encounters `tok[0] ! 42;`
- **THEN** it SHALL parse as a `Stmt::Send` with channel expression `tok[0]` and argument `42`

#### Scenario: Send with variable index

- **WHEN** the parser encounters `tok[i] ! msg;`
- **THEN** it SHALL parse as a `Stmt::Send` with channel expression `tok[i]` and argument `msg`

#### Scenario: Send with _pid index

- **WHEN** the parser encounters `tok[_pid] ! msg;`
- **THEN** it SHALL parse as a `Stmt::Send` with channel expression `tok[_pid]` and argument `msg`

#### Scenario: Send with arithmetic expression index

- **WHEN** the parser encounters `tok[i+1] ! msg;`
- **THEN** it SHALL parse as a `Stmt::Send` with channel expression `tok[i+1]` and argument `msg`

### Requirement: Parse indexed channel access in receive statements

The parser SHALL accept expressions (not just identifiers) as the channel target in receive statements. The syntax `chan[expr] ? target` SHALL be parsed as a receive operation from the channel at index `expr` in the array `chan`.

#### Scenario: Receive with literal index

- **WHEN** the parser encounters `tok[0] ? x;`
- **THEN** it SHALL parse as a `Stmt::Recv` with channel expression `tok[0]` and target `x`

#### Scenario: Receive with variable index

- **WHEN** the parser encounters `tok[i] ? msg;`
- **THEN** it SHALL parse as a `Stmt::Recv` with channel expression `tok[i]` and target `msg`

#### Scenario: Receive with _pid index

- **WHEN** the parser encounters `tok[_pid] ? msg;`
- **THEN** it SHALL parse as a `Stmt::Recv` with channel expression `tok[_pid]` and target `msg`

### Requirement: Generate code for channel arrays

The codegen SHALL emit initialization code for N channels from a channel array declaration. Each channel SHALL be named `<array_name>_<index>` where index ranges from 0 to size-1.

#### Scenario: Channel array initialization

- **WHEN** codegen processes `chan tok[5];`
- **THEN** it SHALL emit state variables: `state.tok_0 = nil`, `state.tok_1 = nil`, ..., `state.tok_4 = nil`
- **AND** it SHALL generate runtime registration for 5 channels: `tok_0`, `tok_1`, `tok_2`, `tok_3`, `tok_4`

#### Scenario: Multiple channel arrays

- **WHEN** codegen processes `chan a[2];` and `chan b[3];`
- **THEN** it SHALL emit: `state.a_0`, `state.a_1`, `state.b_0`, `state.b_1`, `state.b_2`
- **AND** channels SHALL NOT conflict between arrays

### Requirement: Generate code for indexed channel access

The codegen SHALL generate Lua code that dynamically computes the channel name from the index expression. The generated code SHALL call runtime send/recv functions with the computed channel name.

#### Scenario: Indexed send codegen

- **WHEN** codegen processes `tok[i] ! msg;`
- **THEN** it SHALL generate Lua code that:
  1. Evaluates the index expression `i`
  2. Computes channel name: `"tok_" .. tostring(i)`
  3. Calls `runtime_send(channel_name, {msg})`

#### Scenario: Indexed receive codegen

- **WHEN** codegen processes `tok[_pid] ? msg;`
- **THEN** it SHALL generate Lua code that:
  1. Evaluates the index expression `_pid`
  2. Computes channel name: `"tok_" .. tostring(_pid)`
  3. Calls `runtime_recv(channel_name)` and assigns result to `msg`

### Requirement: Runtime channel array registration

The runtime SHALL register N individual channels when processing a channel array declaration. Each channel SHALL be accessible by its generated name (`<name>_<index>`).

#### Scenario: Register channel array

- **WHEN** the runtime processes a `ChannelArray { name: "tok", size: 5 }`
- **THEN** it SHALL register 5 channels: `tok_0`, `tok_1`, `tok_2`, `tok_3`, `tok_4`
- **AND** each channel SHALL have capacity 0 (rendezvous)

#### Scenario: Channel array access

- **WHEN** code calls `runtime_send("tok_2", {42})`
- **THEN** the value 42 SHALL be sent to channel `tok_2`
- **AND** it SHALL block until a receiver receives from `tok_2`

### Requirement: Runtime indexed channel operations

The runtime SHALL support send and receive operations on channels identified by dynamically computed names. The channel name computation SHALL happen at runtime based on the evaluated index expression.

#### Scenario: Dynamic channel send

- **WHEN** Lua code computes `channel_name = "tok_" .. i` where `i = 3`
- **AND** calls `runtime_send(channel_name, {msg})`
- **THEN** the message SHALL be sent to channel `tok_3`

#### Scenario: Dynamic channel receive

- **WHEN** Lua code computes `channel_name = "tok_" .. _pid` where `_pid = 2`
- **AND** calls `runtime_recv(channel_name)`
- **THEN** the receive SHALL happen from channel `tok_2`

### Requirement: Integration test for channel arrays

There SHALL be an integration test that verifies the complete channel array functionality with a realistic model similar to `token_ring_n5`.

#### Scenario: Token ring with channel arrays

- **WHEN** the following model is executed:

  ```promela
  chan tok[3];
  init { tok[0] ! 1 }
  active [3] proctype node() {
      byte msg;
      do :: tok[_pid] ? msg -> tok[(_pid + 1) % 3] ! msg od
  }
  ```

- **THEN** the model SHALL parse without errors
- **AND** the model SHALL execute successfully
- **AND** the token SHALL circulate through all 3 nodes
- **AND** the state count SHALL match expected behavior for a 3-node token ring
