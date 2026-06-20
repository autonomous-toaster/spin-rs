## Task Reference

| Task ID | Description |
|---------|-------------|
| T2.1 | Instrument spin-rs with timing probes at each pipeline stage |
| T2.2 | Measure parse throughput (chars/sec) |
| T2.3 | Measure codegen throughput (AST nodes/sec) |
| T2.4 | Measure Lua bootstrap time |
| T2.5 | Measure guard evaluation throughput (guards/sec) |
| T2.6 | Measure effect execution throughput (effects/sec) |
| T2.7 | Measure state serialization cost (bytes/state, states/sec) |
| T2.8 | Measure hash lookup throughput (fxhash + HashMap ops/sec) |
| T2.9 | Measure Lua↔Rust FFI roundtrip cost |
| T2.10 | Aggregate breakdown and identify bottleneck candidates |

## ADDED Requirements

### Requirement: Instrument spin-rs pipeline stages

T2.1 SHALL complete BEFORE T2.2 through T2.9 SHALL measure individual stages. T2.1 SHALL insert timing probes at each pipeline boundary:

- `Parse` start/end (nom parsing of Promela source)
- `Codegen` start/end (AST traversal to Lua source emission)
- `LuaBootstrap` start/end (mlua instance creation, prelude loading)
- `Verify` start/end (entire state exploration)
- Per-state timing accumulated during verification

T2.1 SHALL use `std::time::Instant` for all timing. T2.1 SHALL NOT modify the public API — instrumentation SHALL be gated behind `#[cfg(feature = "bench")]`.

#### Scenario: Basic instrumentation

- **WHEN** T2.1 instruments the pipeline
- **AND** a model is verified with `--feature bench`
- **THEN** the benchmark harness SHALL print a time breakdown: parse ms, codegen ms, Lua bootstrap ms, verify ms

### Requirement: Parse throughput

T2.2 SHALL measure Promela parsing throughput. T2.2 SHALL run a model through parser::parse() (no codegen, no verification) and report chars/sec. T2.2 SHALL run AFTER T2.1 SHALL add the instrumented parse wrapper.

#### Scenario: Parse throughput report

- **WHEN** T2.2 parses a 10,000-char model
- **THEN** T2.2 SHALL report parse throughput in chars/sec

### Requirement: Codegen throughput

T2.3 SHALL ALWAYS measure Lua code generation throughput. T2.3 SHALL parse a model, then generate Lua from the AST, and report AST nodes/sec. T2.3 SHALL count the total AST nodes processed during codegen.

#### Scenario: Codegen throughput report

- **WHEN** T2.3 parses and generates Lua for a model with 100+ AST nodes
- **THEN** T2.3 SHALL report codegen throughput in AST nodes/sec

### Requirement: Lua bootstrap time

T2.4 SHALL ALWAYS measure the time to create an mlua instance with prelude loaded. T2.4 SHALL NOT include codegen time — it SHALL measure the pre-compiled Lua module loading step used by LuaModel.

#### Scenario: Bootstrap time

- **WHEN** T2.4 loads a Lua model
- **THEN** T2.4 SHALL report the time from mlua::Lua::new() through prelude execution to ready state

### Requirement: Guard evaluation throughput

T2.5 SHALL ALWAYS measure how fast Lua guards evaluate for a given state. T2.5 SHALL call the generated `_spin_get_transitions` function and evaluate each guard, recording total time across all guards evaluated.

#### Scenario: Guard throughput

- **WHEN** T2.5 evaluates all guards across the full state space of a medium model (10K+ states)
- **THEN** T2.5 SHALL report guards evaluated per second

### Requirement: Effect execution throughput

T2.6 SHALL ALWAYS measure how fast Lua effects modify the state vector. T2.6 SHALL call each generated effect function and record total time across all effects executed.

#### Scenario: Effect throughput

- **WHEN** T2.6 executes all effects across the full state space
- **THEN** T2.6 SHALL report effects executed per second

### Requirement: State serialization cost

T2.7 SHALL ALWAYS measure the cost of serializing the Lua state vector to a Rust Vec<u8> for hashing and storage. T2.7 SHALL record bytes/state and states/sec.

#### Scenario: Serialization cost

- **WHEN** T2.7 serializes the state vector after each transition across the full state space
- **THEN** T2.7 SHALL report avg bytes/state and states serialized/sec

### Requirement: Hash lookup throughput

T2.8 SHALL ALWAYS measure fxhash + HashMap insertion throughput. T2.8 SHALL measure raw hash+insert speed by passing pre-serialized state blobs.

#### Scenario: Hash throughput

- **WHEN** T2.8 inserts each visited state into the ExactStore
- **THEN** T2.8 SHALL report hash+insert operations/sec

### Requirement: Lua↔Rust FFI roundtrip cost

T2.9 SHALL ALWAYS measure the cost of a minimal Rust→Lua→Rust roundtrip: calling an empty Lua function that returns immediately. This establishes the noise floor for all Lua-interaction measurements.

#### Scenario: FFI noise floor

- **WHEN** T2.9 calls an empty Lua function 1,000,000 times
- **THEN** T2.9 SHALL report the average roundtrip time in nanoseconds

### Requirement: Aggregate breakdown

T2.10 SHALL combine all per-stage measurements from T2.2 through T2.9 into a single breakdown table. T2.10 SHALL highlight the largest contributors as candidate bottlenecks. T2.10 SHALL complete AFTER all per-stage measurements SHALL complete.

#### Scenario: Bottleneck report

- **WHEN** T2.10 aggregates all measurements for a model
- **THEN** T2.10 SHALL print a table with stage, absolute time, relative share of total, and a "bottleneck?" flag for the top contributor

#### Scenario: Comparison across models

- **WHEN** T2.10 runs the same breakdown across small, medium, and large models
- **THEN** T2.10 SHALL report how each stage scales (constant, linear, super-linear) as model complexity grows
