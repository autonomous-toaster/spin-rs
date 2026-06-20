## Why

The Spin model checker generates C code (`pan.c`) and requires GCC to compile it into a verifier. This ties every Spin-dependent tool — including VeriPlan — to a C compiler toolchain. A Rust-native reimplementation with Promela input and Lua internal runtime eliminates the GCC dependency while keeping the battle-tested Promela surface and enabling in-process embedding as a library.

## What Changes

- New Rust crate `spin-rs`: Promela model checker with embedded Lua runtime
- Promela parser translates `.pml` files to an IR
- IR compiles to Lua scripts (one per proctype) evaluated by mlua (LuaJIT-capable)
- Rust verification engine handles state space exploration, hashing, and property checking
- No dependency on `spin` binary or GCC — everything runs in-process
- Library API for embedding (VeriPlan can `spin_rs::verify(promela, props)` instead of shelling out)
- CLI tool for drop-in comparison with Spin (same flags, same output formats)

## Capabilities

### New Capabilities

- `promela-parser`: Lex and parse Promela (.pml) into an AST with full error reporting
- `lua-codegen`: Compile Promela IR to Lua scripts — state vectors, transition functions, guards, channel operations
- `lua-runtime`: Embedded Lua VM (mlua) executing generated code, bridged to Rust via FFI-safe state representation
- `model-checker`: DFS/BFS state exploration with hash-based state matching, bitstate hashing, and collapse compression
- `property-engine`: LTL formula parsing and Büchi automaton translation (via ω-automata crate), never claim integration, nested DFS for liveness
- `partial-order-reduction`: Selective state space pruning using independence analysis
- `trail-io`: Error trail generation and replay in Spin-compatible format
- `cli`: Command-line tool matching Spin's interface (`spin-rs -a model.pml && spin-rs -run`)
- `library-api`: Rust library API for embedding (`spin_rs::check::Model`, `spin_rs::verify::run`)

### Modified Capabilities

- None (greenfield project)

## Impact

- New crate consuming standard Rust ecosystem (mlua, ω-automata, rayon, tree-sitter)
- VeriPlan will migrate from `spin` subprocess to `spin-rs` library calls
- No changes to existing Spin models — Promela compatibility is a requirement
- No changes to VeriPlan's PlanIR or pipeline — only the backend changes
