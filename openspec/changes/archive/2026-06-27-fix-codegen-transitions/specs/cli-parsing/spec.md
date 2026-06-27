## Task Reference

| Task ID | Description |
|---------|-------------|
| T3.1 | Fix CLI argument parsing in main.rs so flags like `-a` and `--ltl` work |
| T3.2 | Test CLI with `-a` flag produces Lua output |
| T3.3 | Test CLI LTL verification flag |
| T3.4 | Add error message for unrecognized flags |

## ADDED Requirements

### Requirement: Fix argument passing

T3.1 SHALL ALWAYS fix the off-by-one argument parsing bug. Currently `main.rs` calls `spin_rs::cli::run(&args[1..])` which strips the binary name, but `Cli::parse_from` expects the first element to be the program name. T3.1 SHALL pass all args including the binary name to `run()`.

T3.1 SHALL complete BEFORE T3.2 SHALL test the CLI.

#### Scenario: -a flag works

- **WHEN** T3.2 runs `spin-rs -a model.pml`
- **THEN** the CLI SHALL print the generated Lua source code and exit

#### Scenario: -a no longer silently runs verification

- **WHEN** T3.2 runs `spin-rs -a model.pml`
- **THEN** the CLI SHALL NOT run verification (no state exploration output)
