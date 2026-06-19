## Context

Spin model checker compiles Promela to C code (`pan.c`, `pan.h`, `pan.t`) and requires GCC to produce a verifier executable. Every tool that depends on Spin — including VeriPlan — inherits this compiler dependency. Current flow:

```
.pml → spin -a → pan.c → gcc → pan → ./pan → results
```

VeriPlan shells out to `spin` + `gcc` + `pan` as subprocesses, with timeouts per property and hard failure when `spin` is missing from PATH.

This design replaces that chain with an in-process Rust library: Promela in, verification results out. No compiler, no subprocess.

## Goals / Non-Goals

**Goals:**

- Parse standard Promela (Spin 6.5.x compatible subset covering common constructs)
- Compile Promela IR to Lua scripts evaluated by mlua with LuaJIT
- Run DFS/BFS state space exploration with hash-based state matching
- Support safety (assert, never claim) and liveness (LTL, nested DFS) properties
- Produce Spin-compatible error trails and statistics
- Embeddable as `spin_rs::verify()` library — no subprocess
- CLI parity with Spin's basic flags (`-a`, `-run`, `-E`, `-N`)

**Non-Goals:**

- Embedded C code (`c_code`, `c_state`, `c_decl`) — use Lua extension points instead
- Full Spin optimization suite (e.g., swarm verification, multi-core bitstate)
- Remote variable references and full expression syntax expanded
- MSC (message sequence chart) generation
- XSpin GUI integration

## Decisions

### D1: Lua (mlua) as compiler target instead of C

**Decision:** Compile Promela to Lua scripts evaluated by mlua (Lua 5.4 bindings for Rust). LuaJIT can JIT-compile generated functions to near-native speed.

**Why not C codegen:** C codegen re-introduces the GCC dependency we're eliminating.

**Why not Wasm:** Wasm adds FFI overhead per transition call (~50-100ns). With billions of transitions, this compounds. LuaJIT compiles to native machine code within the same process — no FFI boundary.

**Why not direct Rust interpretation (AST walking):** Interpreting Promela's AST directly in Rust would be simpler but slower for transition execution. Lua codegen moves the interpretation cost to LuaJIT's JIT compiler while keeping the architecture clean: generated Lua mirrors what Spin's pangen*.c generates, but for the Lua VM.

### D2: State vector in Rust, transitions in Lua

**Decision:** The global state vector lives in Rust (a `Vec<u8>` or typed struct). Lua receives copies or references to compute next states. The verification engine never enters Lua for bookkeeping — only for model-specific transition enumeration and guard evaluation.

This mirrors Spin's architecture: the `now` vector is C memory, the transition code is generated C. In our case: `now` is a Rust byte vector, transitions are generated Lua functions that read/write it via mlua's FFI.

### D3: Split verification engine into core + model-specific layers

```
spin_rs::engine      → DFS/BFS, hash tables, POR (model-agnostic)
spin_rs::promela::ir → Parsed Promela representation
spin_rs::promela::lua → Promela → Lua compiler
spin_rs::promela::rt → Lua runtime bridge for that model
```

The engine is model-agnostic — it drives state exploration via a trait:

```rust
trait Model {
    type State: Hash + Eq + Clone;
    fn initial_states(&self) -> Vec<Self::State>;
    fn transitions(&self, state: &Self::State) -> Vec<Transition<Self::State>>;
    fn hash(&self, state: &Self::State) -> u64;
}
```

The Lua bridge implements this trait by calling into mlua.

### D4: LTL → Büchi via ω-automata crate

**Decision:** Use the existing `omega-automata` Rust crate for LTL translation instead of reimplementing Spin's tableau algorithm.

This crate translates LTL → VWABW → GBW → NBW (Büchi). The NBW is then used for synchronous product with the model during state exploration (nested DFS for liveness properties). Spin's never claim format is also supported as an alternative input path.

### D5: Promela subset for v1, full language later

**Decision:** v1 supports a practical subset:

- Variables: `bit`, `bool`, `byte`, `short`, `int`, `chan` of basic types
- Statements: assignment, `if/fi`, `do/od`, `goto`, `break`, `assert`, `printf`
- Channels: buffered/unbuffered, synchronous/asynchronous, `!`/`?`
- Expressions: all standard operators, `len()`, `full()`, `nempty()`
- Proctypes: `active`, parameterized, dynamic `run`
- Never claims and LTL formulas
- `atomic` sequences (without embedded d_step nuances)
- `unless` statement

Excluded from v1: inline C code, `d_step`, `c_code`/`c_state`, `provided`, `priority`, `remote` refs, `set`/`reset` on channel poll, `timeout` as expression. These can be added later via Lua extensions.

### D6: Partial order reduction via persistent sets

**Decision:** Implement Spin's persistent-set based POR (ample sets) rather than the more aggressive DPOR. The algorithm is well-understood and documented in Spin's literature. For v1, POR is optional (off by default) — users choose completeness vs. performance.

## Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│  CLI (clap)                                                       │
│  spin-rs -a model.pml      spin-rs -run model.pml                │
└──────────┬──────────────────────────────────────┬─────────────────┘
           │                                      │
           ▼                                      ▼
┌──────────────────────┐  ┌──────────────────────────────┐
│  Promela Parser       │  │  Library API                  │
│  (pest/nom)           │  │  spin_rs::verify(pml, props)  │
│  → AST                │  └──────────┬───────────────────┘
└──────────┬───────────┘             │
           │                         │
           ▼                         ▼
┌──────────────────────────────────────────────────┐
│  Lua Codegen                                      │
│  • State vector layout → Lua table template       │
│  • Per-proctype control flow → Lua closures       │
│  • Guard predicates → Lua boolean expressions     │
│  • Channel operations → Lua C API calls           │
│  • Never claim → Lua Büchi monitor                │
└──────────────────────┬───────────────────────────┘
                       │
                       ▼
┌──────────────────────────────────────────────────┐
│  Lua Runtime (mlua)                               │
│  • Loads generated Lua strings                    │
│  • Exposes Rust state vector as Lua userdata      │
│  • Transition enumeration: Rust calls Lua         │
│  • Guard evaluation: Lua closures return bool     │
└──────────────────────┬───────────────────────────┘
                       │
                       ▼
┌──────────────────────────────────────────────────┐
│  Verification Engine (Rust)                       │
│                                                    │
│  ┌────────────┐  ┌──────────┐  ┌───────────┐     │
│  │ DFS Stack   │  │ Hash     │  │ State     │     │
│  │ (Vec<State>)│  │ Table    │  │ Storage   │     │
│  │             │  │ (DashMap)│  │ (bitstate │     │
│  │ Nested DFS  │  │ Collapse │  │ or exact) │     │
│  └────────────┘  └──────────┘  └───────────┘     │
│                                                    │
│  ┌──────────┐  ┌────────────┐  ┌───────────┐     │
│  │ POR      │  │ LTL Büchi  │  │ Trail     │     │
│  │ (persist │  │ (ω-auto    │  │ Export    │     │
│  │ sets)    │  │  crates)   │  │ (.pml.tr) │     │
│  └──────────┘  └────────────┘  └───────────┘     │
└──────────────────────────────────────────────────┘
```

### Data Flow

```
1. spin-rs -a model.pml
   ── Parse Promela → IR
   ── Compile IR → Lua scripts (in memory)
   ── Write model.lua (cached)

2. spin-rs -run model.pml
   ── Load/compile Lua scripts via mlua
   ── Engine::new(model_impl) with trait from Lua bridge
   ── DFS/BFS exploration:
       for each state:
           lua_rt.transitions(state) → Vec<Transition>
           for each transition:
               lua_rt.apply(state, transition) → next_state
               if !seen(next_state):
                   push(next_state)
   ── Liveness: nested DFS for never claims
   ── Output: error trails, statistics
```

## Risks / Trade-offs

- **[Risk] Lua transition speed vs. C**: LuaJIT can approach C speed for tight loops but may fall short on complex guard expressions. **Mitigation:** Profile common Promela patterns; hot paths can be migrated to Rust if needed.
- **[Risk] Promela compatibility**: Spin's Promela has corner cases (scoping rules, inline expansion, implicit semi-colons). **Mitigation:** Compare against Spin's test suite; precise error messages for unsupported constructs.
- **[Risk] Partial order reduction correctness**: POR is notoriously easy to get wrong. **Mitigation:** Off by default in v1; verified against exhaustive search for validation.
- **[Risk] State explosion**: Same as Spin, no mitigation beyond standard techniques (bitstate, collapse, POR). Memory usage for hash table competes with Spin's.
- **[Trade-off] No embedded C**: Spin's `c_code` lets users embed arbitrary C. Lua extension points replace this but require rewriting embedded C as Lua. For VeriPlan's use case (generated Promela, no embedded C), this is irrelevant.
