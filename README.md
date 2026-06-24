# spin-rs

[![License: WTFPL](https://img.shields.io/badge/License-WTFPL-brightgreen.svg)](http://www.wtfpl.net/)

**A Rust-native Promela model checker with Lua runtime — no GCC dependency required.**

Parse, compile, and verify Promela models entirely in-process. `spin-rs` translates Promela to
Lua, then executes the transitions via `mlua` (embedded Lua 5.4). No C compiler, no subprocesses.

## Architecture

```
Promela source
    │
    ▼
┌──────────────────────────────────────────────────────────┐
│  Parser (nom)                Promela → AST               │
│  • Variables, channels, proctypes, never claims          │
│  • Control flow (if/fi, do/od, goto, break)             │
│  • Channel ops (!/?), LTL inline, c_code passthrough    │
└───────────────────────┬──────────────────────────────────┘
                        │
                        ▼
┌──────────────────────────────────────────────────────────┐
│  LTL → Büchi (ltl2ba-rs)   LTL formula → ω-automaton    │
│  • Full LTL grammar ([] <>, X, U, V, R)                 │
│  • Generalized Büchi automaton construction              │
│  • Product construction for model × property             │
│  • Nested DFS for emptiness checking                     │
└───────────────────────┬──────────────────────────────────┘
                        │
                        ▼
┌──────────────────────────────────────────────────────────┐
│  Code Generator           AST → Lua source              │
│  • State vector layout as Lua table                      │
│  • Per-proctype transition closures (guard + effect)     │
│  • Never claim transitions                                │
│  • Channel send/receive callbacks                        │
│  • Atomic sequences, dynamic process creation (run)      │
└───────────────────────┬──────────────────────────────────┘
                        │
                        ▼
┌──────────────────────────────────────────────────────────┐
│  Lua Runtime (mlua)    Execute transitions via FFI       │
│  • mlua 5.4 with vendored Luabuiltins                    │
│  • Rust-backed channel primitives (callable from Lua)    │
│  • State vector serialization/deserialization            │
└───────────────────────┬──────────────────────────────────┘
                        │
                        ▼
┌──────────────────────────────────────────────────────────┐
│  Model Checker Engine  DFS/BFS + storage + violations    │
│  • Depth-first / breadth-first search                    │
│  • Exact store (HashMap w/ collision resolution)          │
│  • Bitstate hashing (Bloom filter, 2 hash functions)     │
│  • Collapse compression (canonical ordinals)             │
│  • Partial order reduction (stubborn sets)               │
│  • Error trail generation + replay (JSON + Spin format)  │
│  • LTL verification (product + nested DFS)               │
│  • BFS shortest counterexample                           │
└──────────────────────────────────────────────────────────┘
```

## Installation

```bash
git clone https://github.com/autonomous-toaster/spin-rs.git
cd spin-rs
cargo build --release
```

The binary will be at `target/release/spin-rs`.

## Usage

### CLI

```bash
# Verify a Promela model
spin-rs model.pml

# Generate and print Lua verifier code
spin-rs -a model.pml

# LTL property verification
spin-rs --ltl liveness '[]<>(x == 0)' model.pml

# BFS search
spin-rs --search bfs model.pml

# Bitstate hashing (approximate, memory-efficient)
spin-rs --storage bitstate model.pml

# Partial order reduction
spin-rs --por model.pml

# Save error trail
spin-rs --trail-file error.trail model.pml

# Max states / depth
spin-rs --max-states 100000 --max-depth 1000 model.pml
```

### Library

```rust
use spin_rs::verify;

let promela = r#"
    active proctype P() { byte x = 0; x = 1; assert(x == 1); }
"#;
let result = verify(promela)?;
println!("{} states explored", result.states_explored);
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
| LTL formula parser ([] <>, X, U, V, R, ->, &&, \|\|, !) | ✅ |
| LTL → Generalized Büchi automaton | ✅ |
| Product construction (model × property) | ✅ |
| Nested DFS for emptiness checking | ✅ |
| Partial order reduction (stubborn sets, C3) | ✅ |
| Collapse compression (canonical ordinals) | ✅ |
| Error trails (JSON + Spin format) | ✅ |
| Spin trail replay (`spin -t -X`) | ✅ |
| Lua code generation + execution (mlua) | ✅ |
| BFS + shortest counterexample | ✅ |
| Bitstate hashing (Bloom filter) | ✅ |
| Atomic sequences + `d_step` | ✅ |
| Dynamic process creation (`run`) | ✅ |
| Never claims | ✅ |
| Parallel verification (rayon) | ✅ |
| Fairness constraints (weak fairness) | ✅ |

## Promela Support

**Data types**: `bit`, `bool`, `byte`, `short`, `int`, `chan`, arrays  
**Channel arrays**: `chan name[N];` - array of N rendezvous channels with indexed access (`chan[i] ! msg`, `chan[i] ? var`)  
**Control flow**: `if`/`fi`, `do`/`od`, `goto`, `break`, `skip`, `atomic`, `d_step`  
**Channels**: buffered/unbuffered, `!` (send), `?` (receive), poll, sorted  
**Expressions**: arithmetic, comparison, boolean, bitwise, `len`, `enabled`, `remote ref`  
**Properties**: `assert`, LTL inline, named LTL formulas, never claims  
**LTL operators**: `[]` (always), `<>` (eventually), `X` (next), `U` (until), `V` (release), `R` (release)

## Benchmarks

```bash
cargo bench --features bench
```

Run performance benchmarks comparing transition throughput across models.

## License

WTFPL — see [LICENSE](LICENSE).
