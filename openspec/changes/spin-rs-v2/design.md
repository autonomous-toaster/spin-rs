# spin-rs v2 Architecture

## Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        spin-rs v2 Architecture                           │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Promela Source                                                         │
│      │                                                                  │
│      ▼                                                                  │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ Parser (nom)                                                    │   │
│  │ - Promela AST                                                   │   │
│  │ - LTL formulas (inline & never claims)                          │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│      │                                                                  │
│      ▼                                                                  │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ Code Generator                                                  │   │
│  │ - Lua source (state + transitions)                              │   │
│  │ - Metadata (state structure, process boundaries)                │   │
│  │ - Büchi automata (from LTL via omega-automata)                  │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│      │                                                                  │
│      ▼                                                                  │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ Lua Runtime (mlua)                                              │   │
│  │ - State vector (with structure metadata)                        │   │
│  │ - Channel primitives (Rust FFI)                                 │   │
│  │ - Transition enumeration                                        │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│      │                                                                  │
│      ▼                                                                  │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ Model Checker Engine                                            │   │
│  │                                                                 │   │
│  │  ┌──────────────────────────────────────────────────────────┐  │   │
│  │  │ Product Construction (NEW v2)                            │  │   │
│  │  │ - Model state × Büchi state                              │  │   │
│  │  │ - Synchronous product on atomic props                    │  │   │
│  │  └──────────────────────────────────────────────────────────┘  │   │
│  │                                                                 │   │
│  │  ┌──────────────────────────────────────────────────────────┐  │   │
│  │  │ Nested DFS                                               │  │   │
│  │  │ - Outer DFS: reach accepting states                      │  │   │
│  │  │ - Inner DFS: find cycles                                 │  │   │
│  │  │ - C3 condition: sound POR (NEW v2)                       │  │   │
│  │  └──────────────────────────────────────────────────────────┘  │   │
│  │                                                                 │   │
│  │  ┌──────────────────────────────────────────────────────────┐  │   │
│  │  │ Storage Backends                                         │  │   │
│  │  │ - Exact: HashMap<u64, Vec<State>>                        │  │   │
│  │  │ - Bitstate: Bloom filter                                 │  │   │
│  │  │ - Collapse: Per-component canonicalization (NEW v2)      │  │   │
│  │  └──────────────────────────────────────────────────────────┘  │   │
│  │                                                                 │   │
│  │  ┌──────────────────────────────────────────────────────────┐  │   │
│  │  │ Partial Order Reduction                                  │  │   │
│  │  │ - Persistent sets (v1)                                   │  │   │
│  │  │ - C3 cycle detection (NEW v2)                            │  │   │
│  │  │ - Dependency analysis (enhanced)                         │  │   │
│  │  └──────────────────────────────────────────────────────────┘  │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│      │                                                                  │
│      ▼                                                                  │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ Output                                                          │   │
│  │ - CheckResult (states, errors, violations)                      │   │
│  │ - ErrorTrail (JSON + Spin binary format)                        │   │
│  │ - Statistics (Spin-compatible output)                           │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

## Key Architectural Decisions

### 1. LTL → Büchi Pipeline

**Decision**: Use omega-automata crate for LTL → Büchi conversion, construct product space explicitly.

**Rationale**:

- omega-automata implements the state-of-the-art Vardi-Wolper tableau algorithm
- Correctness is critical — don't reimplement complex automata theory
- Product construction gives us full control over synchronization

**Architecture**:

```rust
// v2 flow
LtlFormula::parse("[]<>(x == 0)")
    │
    ▼
omega_ltl::Formula
    │
    ▼
LtlToVWABW::new()  // Very Weak Alternating Büchi
    │
    ▼
VWABWToGBW::new()  // Generalized Büchi (multiple accepting sets)
    │
    ▼
GBWToNBW::new()    // Non-deterministic Büchi (single accepting set)
    │
    ▼
BuchiAutomaton {
    states: Vec<BuchiState>,
    initial: usize,
    accepting: HashSet<usize>,
    transitions: Vec<Vec<BuchiTransition>>,
}
    │
    ▼
ProductConstruction::new(model, buchi)
    │
    └─► ProductState {
            model_state: S,
            buchi_state: usize,
        }
```

**Trade-offs**:

- ✅ Correct by construction (uses proven algorithms)
- ✅ Separation of concerns (automata theory separate from model checking)
- ❌ Extra allocation (Büchi automaton stored separately)
- ❌ Product space larger than model space alone

**Alternative considered**: Implement tableau algorithm directly in spin-rs.

- Rejected: High complexity, risk of bugs, reinventing the wheel

---

### 2. Product Space Exploration

**Decision**: Explicit product construction (model state × Büchi state) rather than implicit on-the-fly product.

**Rationale**:

- Clearer separation: model checker doesn't need to know about LTL semantics
- Easier to debug (can inspect Büchi automaton separately)
- Reusable: same product construction works for any Büchi automaton

**Data structures**:

```rust
/// Product state for nested DFS
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct ProductState<S> {
    /// The model's state representation
    pub model_state: S,
    /// Current Büchi automaton state
    pub buchi_state: usize,
    /// Cached hash for performance
    cached_hash: u64,
}

/// Synchronous product transition
pub struct ProductTransition<S> {
    /// Model transition label
    pub label: String,
    /// Büchi state after transition
    pub next_buchi: usize,
    /// Whether this transition visits an accepting state
    pub is_accepting: bool,
    /// The next product state
    pub next: ProductState<S>,
}
```

**Transition synchronization**:

```rust
// For each model transition, determine which Büchi transitions are enabled
fn sync_transitions(
    model_trans: &Transition<S>,
    buchi_state: usize,
    buchi: &BuchiAutomaton,
    atomic_props: &HashSet<String>,  // Props true in model_trans.next
) -> Vec<usize> {
    buchi.transitions[buchi_state]
        .iter()
        .filter(|b_trans| {
            // Check if all Büchi transition conditions are satisfied
            b_trans.conditions.iter().all(|(prop, must_be_true)| {
                atomic_props.contains(prop) == *must_be_true
            })
        })
        .map(|b_trans| b_trans.to)
        .collect()
}
```

---

### 3. C3 Cycle Condition for POR

**Decision**: Implement stack-synchronous C3 condition — if any state on the DFS stack has an unexpanded transition leading back to the stack, expand all transitions at that state.

**Rationale**:

- Simplest correct C3 implementation
- Matches Spin's approach
- Sound for all LTL properties

**Algorithm**:

```rust
fn ample_set_with_c3<S>(
    state: &S,
    stack: &[ProductState<S>],  // Current DFS path
    transitions: &[Transition<S>],
    deps: &[TransitionDeps],
) -> Vec<usize> {
    // Standard persistent set computation
    let mut ample = compute_persistent_set(transitions, deps);
    
    // C3 check: are we in a cycle?
    let current_hash = hash(state);
    let cycle_start_idx = stack.iter()
        .position(|s| hash(s) == current_hash);
    
    if let Some(idx) = cycle_start_idx {
        // We're in a cycle! Check if any state in the cycle
        // has unexpanded transitions.
        let cycle_has_unexpanded = stack[idx..].iter().any(|stack_state| {
            let trans = model.transitions(stack_state);
            let ample_for_state = compute_persistent_set(&trans, ...);
            ample_for_state.len() < trans.len()  // Some were pruned
        });
        
        if cycle_has_unexpanded {
            // C3 violation! Must expand all transitions at some state in cycle
            // Conservative: expand all at current state
            ample = (0..transitions.len()).collect();
        }
    }
    
    ample
}
```

**Correctness invariant**: If an accepting cycle exists in the full state space, it exists in the reduced state space constructed with C3.

---

### 4. Collapse Compression

**Decision**: Per-process component grouping — each proctype's local variables form a component, globals form a separate component.

**Rationale**:

- Matches Spin's approach
- Simple to implement with existing codegen metadata
- Good compression for typical Promela models (many processes, few globals)

**Codegen metadata**:

```lua
-- Generated Lua includes state structure metadata
function _spin_get_metadata()
    return {
        components = {
            { name = "globals", vars = {"x", "y", "turn"} },
            { name = "P_0", vars = {"x", "pc"} },
            { name = "P_1", vars = {"x", "pc"} },
            { name = "Q_0", vars = {"y"} },
        },
        num_processes = 3,
    }
end
```

**Compression algorithm**:

```rust
pub struct CollapseStore {
    /// Per-component canonical maps: component_id -> (value_vector -> ordinal)
    component_maps: Vec<DashMap<Vec<u8>, usize>>,
    /// Current canonical representation per component
    current_repr: Vec<Vec<u8>>,
}

impl CollapseStore {
    fn insert(&mut self, state: &LuaTable) -> bool {
        // Extract per-component values
        let component_values = self.extract_components(state);
        
        // Canonicalize each component
        let mut canonical_changed = false;
        for (comp_id, values) in component_values.iter().enumerate() {
            let map = &mut self.component_maps[comp_id];
            let ordinal = map.entry(values.clone())
                .or_insert_with(|| {
                    canonical_changed = true;
                    map.len()
                });
            
            self.current_repr[comp_id] = encode_ordinal(*ordinal);
        }
        
        // Check if full canonical representation is new
        let full_repr = self.current_repr.concat();
        self.seen.insert(full_repr)
    }
}
```

**Expected compression**:

- 2 processes, 3 vars each, 1 global: 7 vars → 2 components (3:1 compression)
- 10 processes, 5 vars each, 2 globals: 52 vars → 11 components (5:1 compression)
- Typical: 5-10x memory reduction

---

### 5. Trail Format Compatibility

**Decision**: Support both JSON (v1, human-readable) and Spin binary format (v2, `spin -t` compatible).

**Spin binary format** (reverse-engineered from Spin source):

```
Header:
  - Magic: 0x56455253 ("VERS")
  - Version: uint32
  - States explored: uint64
  - Depth: uint32
  - Num steps: uint32

Per step:
  - Process ID: uint16
  - Statement line: uint16
  - Transition type: uint8
  - Optional data: variable-length
```

**Implementation**:

```rust
pub enum TrailFormat {
    Json,      // v1, human-readable
    SpinBinary, // v2, spin -t compatible
    SpinText,   // v1, human-readable summary
}

impl ErrorTrail {
    pub fn save(&self, path: &Path, format: TrailFormat) -> io::Result<()> {
        match format {
            TrailFormat::Json => self.save_json(path),
            TrailFormat::SpinBinary => self.save_spin_binary(path),
            TrailFormat::SpinText => self.save_spin_text(path),
        }
    }
    
    pub fn save_spin_binary(&self, path: &Path) -> io::Result<()> {
        let mut file = File::create(path)?;
        
        // Write header
        file.write_all(&0x56455253u32.to_le_bytes())?; // Magic
        file.write_all(&2u32.to_le_bytes())?;          // Version
        file.write_all(&(self.states_explored as u64).to_le_bytes())?;
        // ... rest of header
        
        // Write steps
        for step in &self.steps {
            let (pid, line) = self.parse_spin_label(&step.label)?;
            file.write_all(&pid.to_le_bytes())?;
            file.write_all(&line.to_le_bytes())?;
            // ... transition type and data
        }
        
        Ok(())
    }
}
```

---

## Module Structure (v2)

```
src/
├── lib.rs                 # Public API
├── main.rs                # CLI
│
├── parser/
│   ├── mod.rs             # Promela parser
│   ├── ast.rs             # AST types
│   └── ltl.rs             # LTL formula parsing (enhanced)
│
├── codegen/
│   ├── mod.rs             # Lua code generation
│   ├── state.rs           # State layout generation
│   ├── transitions.rs     # Transition enumeration
│   └── metadata.rs        # State structure metadata (NEW)
│
├── runtime/
│   ├── mod.rs             # mlua integration
│   ├── channels.rs        # Channel primitives
│   └── state.rs           # State serialization/deserialization
│
├── engine/
│   ├── mod.rs
│   ├── checker.rs         # DFS/BFS engine
│   ├── product.rs         # Product construction (NEW)
│   ├── nested_dfs.rs      # Nested DFS for LTL (NEW)
│   └── storage/
│       ├── mod.rs
│       ├── exact.rs       # Exact storage
│       ├── bitstate.rs    # Bloom filter
│       └── collapse.rs    # Collapse compression (NEW)
│
├── por/
│   ├── mod.rs             # POR manager
│   ├── deps.rs            # Dependency analysis (enhanced)
│   ├── ample.rs           # Ample set computation
│   └── c3.rs              # C3 cycle condition (NEW)
│
├── property/
│   ├── mod.rs             # Property checking
│   ├── ltl.rs             # LTL → Büchi (NEW, uses omega-automata)
│   ├── buchi.rs           # Büchi automaton types (NEW)
│   ├── never.rs           # Never claims (enhanced)
│   └── fairness.rs        # Fairness constraints (NEW)
│
├── trail/
│   ├── mod.rs             # Trail generation
│   ├── json.rs            # JSON format (v1)
│   ├── spin_binary.rs     # Spin binary format (NEW)
│   └── replay.rs          # Trail replay
│
└── cli/
    └── mod.rs             # CLI (enhanced with new flags)
```

---

## Testing Strategy

### Unit Tests

- LTL parsing: all operators, nested formulas
- Büchi construction: known formulas with known automata sizes
- Product construction: simple models with hand-verified products
- C3 condition: models where POR without C3 would miss violations
- Collapse: compression ratio benchmarks

### Integration Tests

- Spin standard suite: peterson.pml, leader.pml, etc.
- LTL benchmark suite: formulas from literature
- Comparison testing: run same model through Spin and spin-rs, compare results

### Property-Based Testing

- Generate random Promela models, verify both Spin and spin-rs produce same result
- Generate random LTL formulas, check Büchi automata properties

---

## Performance Considerations

### Hot Paths

1. **Product state hashing**: Must be fast (called on every transition)
   - Solution: cached hash in ProductState, incremental update

2. **Büchi transition matching**: Check conditions on every model transition
   - Solution: precompute condition bitmasks, use bit operations

3. **Collapse canonicalization**: Extract and hash per-component values
   - Solution: direct memory access (no Lua serialization in hot path)

### Memory Management

- Büchi automata: typically small (<100 states for most formulas)
- Product space: largest data structure, needs efficient storage
- Collapse: reduces memory but adds CPU overhead (trade-off)

### Parallelization Opportunities

- Büchi construction: embarrassingly parallel (per-formula)
- Nested DFS: inherently sequential (but multiple properties can run in parallel)
- Collapse: per-component canonicalization can be parallel

---

## Migration Path (v1 → v2)

### Breaking Changes

- `CheckResult` gains new fields (büchi_states, product_size)
- `verify()` function unchanged (backward compatible)
- CLI gains new flags (--ltl-file, --fairness, --collapse)

### Non-Breaking

- All v1 Promela models work unchanged
- Existing library API continues to work
- JSON trail format still supported

### Deprecation Warnings (v2.x)

- Simplified LTL checking (without Büchi) deprecated in favor of full pipeline
- Text-only trail format deprecated in favor of JSON + binary

---

**Next**: Create detailed specs for each feature (LTL, POR C3, Collapse, Trail Format).
