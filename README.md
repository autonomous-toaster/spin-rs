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

let result = verify_ltl(promela, "[]<>(x == 0)", "liveness")?;
match result {
    Some(v) => println!("❌ Violation: {}", v.description),
    None    => println!("✅ Property holds"),
}
```

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
**LTL operators**: `[]` (always), `<>` (eventually), `X` (next), `U` (until), `V`/`R` (release)  
**Other**: `printf`, inline expansion, `c_code` passthrough, `run`, `provided`, `priority`

## Architecture

```
Promela source → Parser (nom) → AST → Code Generator → Lua → mlua Runtime → Checker Engine → Result
```

The pipeline is fully in-process: parse Promela, generate Lua transition functions,
execute them via embedded Lua, and explore the state space with the verification engine.

## License

WTFPL — see [LICENSE](LICENSE).
