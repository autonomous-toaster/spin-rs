# spin-rs

[![License: WTFPL](https://img.shields.io/badge/License-WTFPL-brightgreen.svg)](http://www.wtfpl.net/)

**A Rust-native Promela model checker with Lua runtime — no GCC dependency required.**

Parse, compile, and verify Promela models entirely in-process. `spin-rs` translates Promela to
Lua, then executes transitions via `mlua` (embedded Lua 5.4). No C compiler, no subprocesses.

## Quick Start

```bash
# Install
git clone https://github.com/autonomous-toaster/spin-rs.git
cd spin-rs
cargo build --release

# Verify a model
./target/release/spin-rs examples/model.pml

# Interactive simulation (step through your model)
./target/release/spin-rs -i examples/model.pml
```

## Usage Examples

### Basic Verification

```bash
# Verify a Promela model (DFS, exact storage)
spin-rs model.pml

# BFS search (finds shortest counterexample)
spin-rs --search bfs model.pml

# Limit state space
spin-rs --max-states 100000 --max-depth 1000 model.pml
```

### Interactive Simulation

Step through your model transition by transition, like Spin's `-i` mode:

```bash
spin-rs -i model.pml
```

Commands during simulation:

- `<N>` — take transition N
- `b` — step back (undo)
- `i` — inspect current state variables
- `h` — show history
- `q` — quit

### LTL Property Verification

```bash
# Verify a liveness property
spin-rs --ltl liveness '[]<>(x == 0)' model.pml

# Multiple LTL formulas can be inlined in the Promela source
```

### Trail Replay

```bash
# Replay an error trail with state inspection
spin-rs -t -k error.trail model.pml

# With full state dumps at each step
spin-rs -t -k error.trail --inspect model.pml
```

### Optimization Levels

```bash
# Dead variable elimination (removes unused vars from state vector)
spin-rs -o2 model.pml

# Statement merging (combines consecutive deterministic transitions)
spin-rs --opt3 model.pml

# Rendezvous optimization (merges sync send/recv pairs)
spin-rs --opt4 model.pml
```

### Advanced Verification Modes

```bash
# Swarm verification: N parallel workers with varied parameters
spin-rs --swarm 8,1 model.pml

# Parallel BFS with N threads
spin-rs --bfspar 4 model.pml

# Hash-compact storage (memory-efficient, 64-bit hashes)
spin-rs --hc model.pml

# Strong fairness constraints for liveness
spin-rs --strong-fairness model.pml

# Bitstate hashing (Bloom filter, approximate)
spin-rs --storage bitstate model.pml

# Partial order reduction
spin-rs --por model.pml
```

### Library API

```rust
use spin_rs::verify;

let promela = r#"
    active proctype P() { byte x = 0; x = 1; assert(x == 1); }
"#;
let result = verify(promela)?;
println!("{} states explored", result.states_explored);
```

```rust
use spin_rs::{CheckerBuilder, StorageMode, SearchMode};

let model = spin_rs::create_model(promela)?;
let checker = CheckerBuilder::new()
    .model(model)
    .max_states(1_000_000)
    .search_mode(SearchMode::BreadthFirst)
    .storage_mode(StorageMode::Bitstate)
    .build();
let result = checker.check();
```

```rust
use spin_rs::property::verify_ltl;

// Supported shapes: p, !p, []p, <>p, Xp and [] (a -> <> b).
let result = verify_ltl(promela, "[] (x == 0 -> <> (done))", "liveness")?;
match result {
    Some(v) => println!("❌ Violation: {}", v.description),
    None    => println!("✅ Property holds"),
}
```

A shape the automaton builder cannot complement returns `Err`, never a green verdict.
See **Verification results** below.

### Verification results

A result is only trustworthy when it says so. Three outcomes are distinct:

| Outcome | How it is reported | Meaning |
|---|---|---|
| No violation | `Ok(result)` with `errors == 0` and `unverified.is_empty()` | the state space closed and every property was decided |
| Violation | `Ok(result)` with `errors > 0` | definite refutation, with a trail |
| Unverified | `Err(...)`, or `Ok(result)` with `unverified` non-empty | no verdict was produced |

`verify` returns `Err` when the model could not be explored or evaluated:

- no process is enabled in the initial state, so nothing was explored;
- an atomic proposition does not name a variable in the model;
- a generated guard or effect failed to evaluate.

All three mean *I could not check this*, not *this is invalid*. Mapping `Err` to a failure
verdict invents a violation.

`CheckResult::unverified` carries the same idea for individual properties: a property the
checker cannot decide (an unsupported LTL shape, or an unparsable formula) is listed there
rather than counted as an error. A model declaring proctypes but exploring zero transitions
is an error, never a pass.

`verify` has no wall-clock deadline. It runs until the state space closes or `max_states`
is reached; use `verify_with_config` to set the budget. A search that stops at the limit
adds a `model` entry to `unverified` saying so, so a truncated run can no longer be
confused with a completed one.

Properties are evaluated per named claim. A caller that checks only the first claim of a
multi-property model will under-report violations; `pan` has the same behaviour and needs
`-N <claim>` per property.

### Why 0.2.0 changed old verdicts

Upgrading from 0.1.0 matters because some earlier verdicts were wrong, not merely different.
Four defects produced them: the state-blob reader returned the *next* variable's value, so
implication-shaped safety properties were judged against the wrong atom; `&&` never matched
across a space, collapsing every conjunction into one unresolvable atom that silently read
as false; mlua maps numeric `0` to `true`, so guards that evaluated to zero were treated as
enabled; and `evaluate_atomic_props` reinterpreted the state pointer as a different type
through `unsafe`, which is undefined behaviour. Liveness verdicts were additionally inverted
and now judge the property's negation.

## Features

| Feature | Status |
|---------|--------|
| Promela parser (variables, channels, processes, control flow) | ✅ |
| LTL formula parser + Büchi automaton | ✅ |
| DFS / BFS state exploration | ✅ |
| Interactive simulation (`-i`) | ✅ |
| Trail replay with state inspection (`-t -k --inspect`) | ✅ |
| Spin-compatible trail format (read/write) | ✅ |
| Swarm verification (`--swarm N,M`) | ✅ |
| Parallel BFS (`--bfspar N`) | ✅ |
| Hash-compact storage (`--hc`) | ✅ |
| Strong fairness constraints (`--strong-fairness`) | ✅ |
| Dead variable elimination (`-o2`) | ✅ |
| Statement merging (`--opt3`) | ✅ |
| Rendezvous optimization (`--opt4`) | ✅ |
| Partial order reduction (stubborn sets) | ✅ |
| Bitstate hashing (Bloom filter) | ✅ |
| Collapse compression | ✅ |
| Error trails (JSON + Spin format) | ✅ |
| LTL verification (product + nested DFS) | ✅ |
| Atomic sequences + `d_step` | ✅ |
| Dynamic process creation (`run`) | ✅ |
| Never claims | ✅ |
| Parallel verification (rayon) | ✅ |
| Fairness constraints (weak + strong) | ✅ |

## Promela Support

**Data types**: `bit`, `bool`, `byte`, `short`, `int`, `chan`, arrays, mtype, typedef  
**Channel arrays**: `chan name[N];` with indexed access  
**Control flow**: `if`/`fi`, `do`/`od`, `goto`, `break`, `skip`, `atomic`, `d_step`, `unless`  
**Channels**: buffered/unbuffered, `!` (send), `?` (receive), poll, sorted, rendezvous  
**Expressions**: arithmetic, comparison, boolean, bitwise, `len`, `enabled`, `full`, `empty`, remote refs  
**Properties**: `assert`, LTL inline, named LTL formulas, never claims, `np_` (non-progress)  
**LTL operators**: `[]`, `<>`, `X` on atomic or negated-atomic operands, plus `[] (a -> <> b)`. `U`, `V`/`R` and nested temporal operators are rejected as unsupported rather than mis-verdicts.  
**Other**: `printf`, inline expansion, `c_code` passthrough, `run`, `provided`, `priority`

## Architecture

```
Promela source → Parser (nom) → AST → Code Generator → Lua → mlua Runtime → Checker Engine → Result
```

The pipeline is fully in-process: parse Promela, generate Lua transition functions,
execute them via embedded Lua, and explore the state space with the verification engine.

## License

WTFPL — see [LICENSE](LICENSE).
