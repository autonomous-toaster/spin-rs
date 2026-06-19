# spin-rs

[![Crates.io](https://img.shields.io/crates/v/spin-rs.svg)](https://crates.io/crates/spin-rs)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Tests](https://github.com/autonomous-toaster/spin-rs/actions/workflows/test.yml/badge.svg)](https://github.com/autonomous-toaster/spin-rs/actions)

**A Rust-native Promela model checker with Lua runtime — no GCC dependency required.**

## Overview

`spin-rs` is a complete reimplementation of the [Spin model checker](https://github.com/spinframework/spin) in Rust. It accepts standard Promela input and performs verification entirely in-process, eliminating the need for GCC compilation.

### Why spin-rs?

The original Spin model checker works by:

1. Parsing Promela source
2. Generating C code (`pan.c`)
3. Compiling with GCC (`gcc -o pan pan.c`)
4. Running the compiled binary (`./pan -N p0`)

This workflow has several drawbacks:

- **GCC dependency**: Requires a C compiler on the target system
- **Subprocess overhead**: Spawns external processes for each verification
- **Slow iteration**: Compilation step adds latency
- **Hard to embed**: Cannot be used as a library in other Rust tools

`spin-rs` solves these problems by:

- **No GCC**: Compiles Promela to Lua bytecode, executed in-process via `mlua`
- **Library-first**: Designed as both a CLI tool and a Rust library
- **Fast iteration**: No compilation step — parse → verify immediately
- **Embeddable**: Call `spin_rs::verify()` directly from your Rust code

### Architecture

```
Promela source
    │
    ▼
┌─────────────────┐
│  Parser (nom)   │  ← Promela → AST
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Code Generator  │  ← AST → Lua source
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Lua Runtime    │  ← mlua 5.4 + Rust FFI
│  (mlua + FFI)   │     - Channel primitives
└────────┬────────┘     - State serialization
         │
         ▼
┌─────────────────┐
│ Model Checker   │  ← DFS/BFS + POR
│  Engine         │     - Exact/Bitstate storage
└────────┬────────┘     - Nested DFS for LTL
         │
         ▼
┌─────────────────┐
│  CheckResult    │  ← States, errors, trails
└─────────────────┘
```

## Installation

### From Source

```bash
git clone https://github.com/autonomous-toaster/spin-rs.git
cd spin-rs
cargo build --release
```

The binary will be at `target/release/spin-rs`.

### As a Library

Add to your `Cargo.toml`:

```toml
[dependencies]
spin-rs = "0.1.0"
```

## Usage

### Command-Line Interface

#### Basic Verification

```bash
# Verify a Promela model
spin-rs model.pml

# With verbose output
spin-rs -v model.pml
```

#### Generate Lua Code

```bash
# Generate and print the Lua verifier
spin-rs -a model.pml
```

#### LTL Property Verification

```bash
# Verify an LTL property
spin-rs --ltl p0 '[]<>(x == 0)' model.pml

# Common LTL patterns:
# []p      - Always p (safety)
# <>p      - Eventually p
# []<>p    - Infinitely often p (liveness)
# p U q    - p until q
```

#### Advanced Options

```bash
# Use BFS instead of DFS
spin-rs --search bfs model.pml

# Use bitstate hashing (faster, approximate)
spin-rs --storage bitstate model.pml

# Enable partial order reduction
spin-rs --por model.pml

# Limit state space
spin-rs --max-states 100000 --max-depth 1000 model.pml

# Save error trail
spin-rs --trail-file error.trail model.pml
```

### Library API

#### Simple Verification

```rust
use spin_rs::verify;

fn main() -> anyhow::Result<()> {
    let promela = r#"
        active proctype P() {
            byte x = 0;
            x = 1;
            assert(x == 1);
        }
    "#;

    let result = verify(promela)?;
    println!("States explored: {}", result.states_explored);
    println!("Errors: {}", result.errors);
    
    Ok(())
}
```

#### Advanced Configuration

```rust
use spin_rs::{
    CheckerBuilder, LuaModel, 
    StorageMode, SearchMode
};

fn main() -> anyhow::Result<()> {
    let promela = std::fs::read_to_string("model.pml")?;
    let model = LuaModel::from_source(&promela)?;

    let checker = CheckerBuilder::new()
        .model(model)
        .max_states(1_000_000)
        .max_depth(100_000)
        .storage_mode(StorageMode::Exact)
        .search_mode(SearchMode::DepthFirst)
        .por_enabled(true)
        .check_assertions(true)
        .build();

    let result = checker.check();
    
    if result.errors > 0 {
        println!("Found {} errors!", result.errors);
        for (i, v) in result.violations.iter().take(5).enumerate() {
            println!("Error {}: {}", i + 1, v.property_name);
            println!("  {}", v.description);
        }
    } else {
        println!("✅ Verification successful!");
    }

    Ok(())
}
```

#### LTL Verification

```rust
use spin_rs::property::verify_ltl;

fn main() -> anyhow::Result<()> {
    let promela = r#"
        byte x = 0;
        active proctype P() {
            do
            :: x = 0
            :: x = 1
            od
        }
    "#;

    // Check liveness: "x is always eventually 0"
    let violation = verify_ltl(promela, "[]<>(x == 0)", "liveness")?;
    
    match violation {
        Some(v) => {
            println!("❌ Property violated: {}", v.property_name);
            println!("Trail: {:?}", v.trail);
        }
        None => println!("✅ Property holds"),
    }

    Ok(())
}
```

#### Error Trail Replay

```rust
use spin_rs::{verify, trail::{ErrorTrail, TrailReplayer, TrailStats}};

fn main() -> anyhow::Result<()> {
    let promela = r#"
        active proctype P() {
            byte x = 0;
            x = 1;
            assert(x == 0); // This will fail
        }
    "#;

    let result = verify(promela)?;
    
    if let Some(violation) = result.violations.first() {
        // Create trail
        let trail = ErrorTrail::new(
            violation.clone(),
            vec![], // state hashes collected during DFS
            result.states_explored,
            result.depth_reached,
        );

        // Save to file
        trail.save_spin_format(std::path::Path::new("error.trail"))?;
        
        // Print statistics
        let stats = TrailStats::compute(&trail);
        stats.print_spin_format();
    }

    Ok(())
}
```

## Promela Support

### Supported Features

✅ **Data types**: `bit`, `bool`, `byte`, `short`, `int`, `chan`  
✅ **Control flow**: `if`/`fi`, `do`/`od`, `goto`, `break`, `skip`  
✅ **Channels**: buffered/unbuffered, `!` (send), `?` (receive)  
✅ **Operators**: arithmetic, comparison, boolean, bitwise  
✅ **Assertions**: `assert()`, safety properties  
✅ **LTL**: `[]`, `<>`, `X`, `U`, `V`, `->`, `&&`, `||`, `!`  
✅ **Never claims**: inline LTL formulas  
✅ **Atomic sequences**: `atomic { ... }`  
✅ **Process types**: `proctype`, `active`, parameterized processes  
✅ **Arrays**: fixed-size arrays  

### Known Limitations

❌ Embedded C code (`c_code { ... }`)  
❌ `d_step` (deterministic step)  
❌ Remote references (`P@x`)  
❌ Priority scheduling  
❌ Process priorities  
❌ Timeout in guards (partially supported)  

## Examples

### Mutual Exclusion (Peterson's Algorithm)

```promela
#define N 2
byte turn = 0;
byte flag[N] = 0;

active [N] proctype user() {
    do
    :: flag[_pid] = 1;
       turn = 1 - _pid;
       (flag[1-_pid] == 0 || turn == _pid);
       /* critical section */
       flag[_pid] = 0;
    od
}

ltl mutual_exclusion {
    []!(user[0]@critical && user[1]@critical)
}
```

### Producer-Consumer

```promela
chan buffer = [5] of { byte };

active proctype Producer() {
    byte i = 0;
    do
    :: i < 10 ->
       buffer ! i;
       i = i + 1
    od
}

active proctype Consumer() {
    byte x;
    do
    :: buffer ? x;
       printf("Received: %d\n", x)
    od
}
```

### Traffic Light Controller

```promela
mtype = { RED, YELLOW, GREEN };
mtype light = RED;

active proctype Controller() {
    do
    :: light == RED -> light = GREEN
    :: light == GREEN -> light = YELLOW
    :: light == YELLOW -> light = RED
    od
}

ltl safety { [](light != GREEN || light != RED) }
ltl liveness { []<>(light == GREEN) }
```

## Performance

`spin-rs` uses several optimization techniques:

- **Partial Order Reduction**: Explores only a subset of interleavings when transitions are independent
- **Bitstate Hashing**: Bloom filter-based storage for large state spaces (trades completeness for memory)
- **Collapse Compression**: Canonical state representation for reduced memory footprint
- **DFS/BFS**: Choice of search strategy based on property type

## Testing

Run the test suite:

```bash
cargo test
```

This includes:

- Parser tests (Promela grammar)
- Codegen tests (Lua generation)
- Runtime tests (mlua integration)
- Engine tests (DFS/BFS, storage modes)
- Property tests (LTL parsing, nested DFS)
- POR tests (independence, ample sets)
- Integration tests (standard Spin models)

## Comparison with Spin

| Feature | Spin 6.5.x | spin-rs 0.1 |
|---------|------------|-------------|
| Input language | Promela | Promela (subset) |
| Runtime | C (compiled) | Lua (interpreted) |
| Dependencies | GCC required | None (vendored Lua) |
| Interface | CLI only | CLI + Library |
| Embeddable | No | Yes (Rust) |
| POR | Yes | Yes (v1 basic) |
| LTL | Yes | Yes (v2 full) |
| Trails | Binary | JSON + Text |
| Parallel | No | Future (v2) |

## Contributing

Contributions are welcome! Areas for improvement:

1. **Full LTL → Büchi**: Integrate ω-automata for complete LTL support
2. **Advanced POR**: Stubborn sets, target sets
3. **Embedded C**: Lua FFI for C code blocks
4. **Parallel verification**: Multi-core state exploration
5. **Spin trail compatibility**: Binary format for `spin -t` replay

## License

MIT License — see [LICENSE](LICENSE) for details.

## Acknowledgments

- [Spin model checker](https://github.com/spinframework/spin) by Gerard J. Holzmann
- [mlua](https://github.com/khvzak/mlua) for Rust ↔ Lua bindings
- [nom](https://github.com/rust-bakery/nom) for parser combinators
- [ω-automata](https://crates.io/crates/omega-automata) for LTL processing

---

**Happy verifying!** 🚀
