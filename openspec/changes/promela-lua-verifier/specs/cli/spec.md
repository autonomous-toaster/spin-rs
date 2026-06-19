## Task Reference

| Task ID | Description |
|---------|-------------|
| T8.1 | Provide CLI with Spin-compatible flags |
| T8.2 | Generate and compile verifier (spin-rs -a) |
| T8.3 | Run verification (spin-rs -run) |

## ADDED Requirements

### Requirement: CLI flag parity

T8.1 SHALL implement a command-line interface with Spin-compatible flags: `-a` (generate verifier), `-run` (compile and run verifier), `-N <name>` (check specific LTL property), `-E` (safety-only mode), `-D<flag>` (define compile-time flag), `-w<N>` (hash table size), `-m<N>` (max depth), `-p` (print all process actions), `-l` (print local variable values), `-g` (print global variable values). T8.1 SHALL complete BEFORE T8.2 SHALL generate the verifier.

#### Scenario: Generate verifier

- **WHEN** user runs `spin-rs -a model.pml`
- **THEN** T8.1 SHALL parse the Promela, generate Lua, and write model.lua

#### Scenario: Run verification

- **WHEN** user runs `spin-rs -run model.pml`
- **THEN** T8.1 SHALL load the generated Lua, explore the state space, and print results

### Requirement: Generate verifier

T8.2 SHALL produce a standalone Lua file (`model.lua`) that can be distributed without the original Promela. The generated Lua SHALL contain all transition logic and state layout needed for verification. T8.2 SHALL complete AFTER T8.1 SHALL parse arguments.

#### Scenario: Generated output

- **WHEN** T8.2 runs `spin-rs -a model.pml`
- **THEN** T8.2 SHALL write `model.lua` in the same directory

### Requirement: Run verification

T8.3 SHALL load the generated Lua and execute the verification engine, printing results to stdout. T8.3 SHALL support all storage modes (`-bitstate`, `-collapse`, default exact). T8.3 SHALL complete AFTER T8.2 SHALL generate the verifier.

#### Scenario: Successful verification

- **WHEN** T8.3 runs on a correct model
- **THEN** T8.3 SHALL print `errors: 0` and summary statistics

#### Scenario: Violation detected

- **WHEN** T8.3 finds a property violation
- **THEN** T8.3 SHALL print `errors: 1` and write a `.pml.trail` file
