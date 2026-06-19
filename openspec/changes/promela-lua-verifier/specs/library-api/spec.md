## Task Reference

| Task ID | Description |
|---------|-------------|
| T9.1 | Provide Rust library API for embedding |
| T9.2 | Accept Promela source or file path |
| T9.3 | Return structured verification results |

## ADDED Requirements

### Requirement: Library API

T9.1 SHALL expose a public Rust API for embedding BEFORE T9.2 SHALL accept Promela source. The API SHALL include at minimum:

- `spin_rs::CheckerBuilder` — builder pattern for configuration
- `spin_rs::Checker` — the verification engine
- `spin_rs::Result` — structured verification output

#### Scenario: Library usage

- **WHEN** a Rust program calls `CheckerBuilder::new().promela(promela_str).build()?.check()`
- **THEN** T9.1 SHALL parse, compile, explore, and return results without spawning any subprocess

### Requirement: Accept Promela input

T9.2 SHALL accept Promela as a string (in-memory) and as a file path. This enables VeriPlan to pass generated Promela without writing to disk. T9.2 SHALL complete AFTER T9.1 SHALL expose the API.

#### Scenario: In-memory Promela

- **WHEN** T9.2 receives a Promela string `"active proctype P() { ... }"` via the library API
- **THEN** T9.2 SHALL parse it without writing any temporary files

### Requirement: Structured results

T9.3 SHALL return structured verification results: pass/fail per property, error trails as Vec<State>, statistics, and violation descriptions. T9.3 SHALL complete AFTER T9.2 SHALL accept Promela and AFTER verification runs.

#### Scenario: Results with violations

- **WHEN** T9.3 returns after a failed verification
- **THEN** T9.3 SHALL include a list of violated properties, each with its error trail and source location annotations
