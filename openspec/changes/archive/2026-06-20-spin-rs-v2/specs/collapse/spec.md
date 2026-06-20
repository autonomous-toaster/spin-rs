# Collapse Compression

## Overview

Implement collapse compression to reduce memory usage by storing per-component canonical representations instead of full state vectors. This enables verifying models with 5-10x larger state spaces in the same memory.

## Requirements

### Functional Requirements

#### R1: Component Identification

- **R1.1**: Identify state components from codegen metadata (per-process variables, globals)
- **R1.2**: Support dynamic component count (depends on model)
- **R1.3**: Support variable-length components (different processes may have different local vars)

#### R2: Canonicalization

- **R2.1**: Map each component's value vector to a canonical ordinal
- **R2.2**: Reuse ordinals for identical component values across states
- **R2.3**: Maintain per-component canonical maps

#### R3: State Storage

- **R3.1**: Store collapsed state (concatenated ordinals) instead of full state
- **R3.2**: Support hash-based deduplication of collapsed states
- **R3.3**: Support reconstruction of full state from collapsed form (for debugging)

#### R4: Integration

- **R4.1**: Integrate with existing `StorageMode::Collapse` enum
- **R4.2**: Work with exact and bitstate modes (collapse is orthogonal)
- **R4.3**: Transparent to model checker engine (same interface as other storage modes)

### Non-Functional Requirements

#### R5: Performance

- **R5.1**: Canonicalization overhead < 10μs per state
- **R5.2**: Memory savings ≥ 5x for typical models (10+ processes)
- **R5.3**: No allocation in hot path (reuse buffers)

#### R6: Correctness

- **R6.1**: Collapsed state uniquely identifies full state (no collisions)
- **R6.2**: Canonicalization is deterministic (same state → same collapsed form)
- **R6.3**: Preserve state equality and hashing semantics

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Collapse Compression Architecture                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Full State Vector (from Lua)                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ globals: { x: 1, y: 2, turn: 0 }                            │   │
│  │ process[0]: { pc: 5, x: 1, flag: true }                     │   │
│  │ process[1]: { pc: 3, x: 2, flag: false }                    │   │
│  │ process[2]: { pc: 7, x: 1, flag: true }                     │   │
│  └─────────────────────────────────────────────────────────────┘   │
│      │                                                              │
│      ▼                                                              │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ Component Extraction                                        │   │
│  │                                                             │   │
│  │  Component 0 (globals):     [1, 2, 0]                       │   │
│  │  Component 1 (process[0]):  [5, 1, 1]                       │   │
│  │  Component 2 (process[1]):  [3, 2, 0]                       │   │
│  │  Component 3 (process[2]):  [7, 1, 1]                       │   │
│  └─────────────────────────────────────────────────────────────┘   │
│      │                                                              │
│      ▼                                                              │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ Per-Component Canonicalization                              │   │
│  │                                                             │   │
│  │  Comp 0 map:           Comp 1 map:                          │   │
│  │    [1,2,0] → 0         [5,1,1] → 0                          │   │
│  │    [0,0,0] → 1         [3,2,0] → 1                          │   │
│  │    ...                 [7,1,1] → 2                          │   │
│  │                        ...                                  │   │
│  │                                                             │   │
│  │  Comp 2 map:           Comp 3 map:                          │   │
│  │    [3,2,0] → 0         [7,1,1] → 0                          │   │
│  │    [5,1,1] → 1         [3,2,0] → 1                          │   │
│  │    ...                 ...                                  │   │
│  └─────────────────────────────────────────────────────────────┘   │
│      │                                                              │
│      ▼                                                              │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ Collapsed State                                             │   │
│  │                                                             │   │
│  │  [ordinal₀, ordinal₁, ordinal₂, ordinal₃]                  │   │
│  │  = [0, 0, 0, 0]                                            │   │
│  │                                                             │   │
│  │  Stored in visited set                                     │   │
│  │                                                             │   │
│  │  Memory: 4 × 4 bytes = 16 bytes                            │   │
│  │  vs. full state: ~50 bytes                                 │   │
│  │  Compression: ~3x for this example                         │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Data Structures

### CollapseStore

```rust
/// Collapse compression storage
pub struct CollapseStore<S> {
    /// Per-component canonical maps: component_id -> (value_vector -> ordinal)
    component_maps: Vec<DashMap<Vec<u8>, usize>>,
    
    /// Current ordinal per component (cached for performance)
    current_ordinals: Vec<usize>,
    
    /// Seen collapsed states (concatenated ordinals)
    seen: HashSet<Vec<usize>>,
    
    /// State count
    count: usize,
    
    /// Metadata: which variables belong to which component
    component_metadata: Vec<ComponentInfo>,
    
    _marker: PhantomData<S>,
}

pub struct ComponentInfo {
    /// Component name (e.g., "globals", "P_0", "P_1")
    pub name: String,
    /// Variable names in this component
    pub vars: Vec<String>,
    /// Byte offsets for each variable in the serialized state
    pub offsets: Vec<usize>,
}

impl<S: Clone + Hash + Eq + Send> CollapseStore<S> {
    pub fn new(metadata: Vec<ComponentInfo>) -> Self {
        let num_components = metadata.len();
        Self {
            component_maps: (0..num_components)
                .map(|_| DashMap::new())
                .collect(),
            current_ordinals: vec![0; num_components],
            seen: HashSet::new(),
            count: 0,
            component_metadata: metadata,
            _marker: PhantomData,
        }
    }
    
    /// Insert a state, returning true if newly inserted
    pub fn insert(&mut self, state: &S) -> bool {
        // Step 1: Extract per-component values
        let component_values = self.extract_components(state);
        
        // Step 2: Canonicalize each component
        for (comp_id, values) in component_values.iter().enumerate() {
            let map = &mut self.component_maps[comp_id];
            
            // Get or create ordinal for this component value
            let ordinal = map.entry(values.clone())
                .or_insert_with(|| map.len());
            
            self.current_ordinals[comp_id] = *ordinal;
        }
        
        // Step 3: Check if collapsed state is new
        let collapsed = self.current_ordinals.clone();
        if self.seen.insert(collapsed) {
            self.count += 1;
            true
        } else {
            false
        }
    }
    
    /// Extract per-component values from state
    fn extract_components(&self, state: &S) -> Vec<Vec<u8>> {
        // This requires model-specific logic
        // For LuaModel, we'd deserialize the Lua table
        // and extract values based on component_metadata
        
        // Simplified example:
        let mut components = Vec::new();
        
        for comp_info in &self.component_metadata {
            let mut values = Vec::new();
            
            for var_name in &comp_info.vars {
                let value = self.get_variable_value(state, var_name);
                values.extend(value);
            }
            
            components.push(values);
        }
        
        components
    }
    
    fn get_variable_value(&self, state: &S, var_name: &str) -> &[u8] {
        // Extract variable value from state
        // For LuaModel: deserialize Lua table, find key, serialize value
        // ...
        &[]
    }
    
    pub fn len(&self) -> usize {
        self.count
    }
    
    /// Get compression ratio
    pub fn compression_ratio(&self) -> f64 {
        if self.component_maps.is_empty() {
            return 1.0;
        }
        
        let total_ordinals: usize = self.component_maps.iter()
            .map(|m| m.len())
            .sum();
        
        let max_states = total_ordinals.pow(self.component_maps.len() as u32);
        max_states as f64 / self.count as f64
    }
}
```

### Codegen Metadata

```rust
// In codegen/mod.rs

/// State structure metadata for collapse compression
pub struct StateMetadata {
    /// Components (per-process groups)
    pub components: Vec<ComponentInfo>,
    /// Number of processes
    pub num_processes: usize,
}

impl LuaGenerator {
    /// Generate state metadata alongside Lua code
    pub fn generate_metadata(&self, model: &PromelaModel) -> StateMetadata {
        let mut components = Vec::new();
        
        // Component 0: globals
        let global_vars: Vec<String> = model.declarations.iter()
            .filter_map(|decl| match decl {
                TopLevel::GlobalVar(v) => Some(v.name.clone()),
                _ => None,
            })
            .collect();
        
        if !global_vars.is_empty() {
            components.push(ComponentInfo {
                name: "globals".to_string(),
                vars: global_vars,
                offsets: vec![], // Computed at runtime
            });
        }
        
        // Components for each proctype instance
        for (pid, decl) in model.declarations.iter().enumerate() {
            if let TopLevel::Proctype(p) = decl {
                let local_vars: Vec<String> = p.body.iter()
                    .filter_map(|stmt| match stmt {
                        Stmt::VarDecl(v) => Some(v.name.clone()),
                        _ => None,
                    })
                    .collect();
                
                // Always include program counter
                let mut vars = vec!["pc".to_string()];
                vars.extend(local_vars);
                
                components.push(ComponentInfo {
                    name: format!("{}_{}", p.name, pid),
                    vars,
                    offsets: vec![],
                });
            }
        }
        
        StateMetadata {
            components,
            num_processes: model.declarations.iter()
                .filter(|d| matches!(d, TopLevel::Proctype(p) if p.active))
                .count(),
        }
    }
}
```

### Integration with Model Checker

```rust
/// Storage mode enum (enhanced)
pub enum StorageMode {
    Exact,
    Bitstate,
    Collapse { metadata: Vec<ComponentInfo> },
}

/// State store trait (enhanced)
pub trait StateStore<S> {
    fn insert(&mut self, hash: u64, state: &S) -> bool;
    fn len(&self) -> usize;
}

impl<S: Clone + Hash + Eq + Send> StateStore<S> for CollapseStore<S> {
    fn insert(&mut self, _hash: u64, state: &S) -> bool {
        // Ignore hash, use collapsed representation
        self.insert(state)
    }
    
    fn len(&self) -> usize {
        self.count
    }
}

/// Checker with collapse support
impl<M: Model> Checker<M> {
    fn make_storage(&self) -> Box<dyn StateStore<M::State>> {
        match self.config.storage_mode {
            StorageMode::Exact => Box::new(ExactStore::new()),
            StorageMode::Bitstate => Box::new(BitstateStore::new(
                (self.config.max_states / 8).max(1024)
            )),
            StorageMode::Collapse { ref metadata } => {
                Box::new(CollapseStore::new(metadata.clone()))
            }
        }
    }
}
```

## Compression Examples

### Peterson's Algorithm (2 processes)

```
Full state:
  globals: { turn: 0, flag[0]: 0, flag[1]: 0 }
  P_0: { pc: 1, x: 0 }
  P_1: { pc: 3, x: 1 }

Components:
  Comp 0 (globals): [0, 0, 0] → ordinal 0
  Comp 1 (P_0):     [1, 0]    → ordinal 0
  Comp 2 (P_1):     [3, 1]    → ordinal 0

Collapsed: [0, 0, 0]

Memory:
  Full: 7 variables × 4 bytes = 28 bytes
  Collapsed: 3 ordinals × 4 bytes = 12 bytes
  Compression: 2.3x
```

### 10 Processes, 5 vars each

```
Full state:
  globals: 2 vars
  P_0..P_9: 5 vars each = 50 vars
  Total: 52 vars × 4 bytes = 208 bytes

Components:
  Comp 0 (globals): 2 vars → 2^16 possible values → ordinal
  Comp 1..10 (processes): 5 vars each → 2^20 possible values each → ordinal

Collapsed: 11 ordinals × 4 bytes = 44 bytes
Compression: 4.7x

Typical reachable states: ~100,000
Full storage: 100k × 208 bytes = 20.8 MB
Collapse storage: 100k × 44 bytes = 4.4 MB
```

## Testing

### Unit Tests

```rust
#[test]
fn test_collapse_component_extraction() {
    let metadata = vec![
        ComponentInfo {
            name: "globals".to_string(),
            vars: vec!["x".to_string(), "y".to_string()],
            offsets: vec![],
        },
        ComponentInfo {
            name: "P_0".to_string(),
            vars: vec!["pc".to_string(), "local".to_string()],
            offsets: vec![],
        },
    ];
    
    let mut store = CollapseStore::new(metadata);
    
    // Insert states with same globals, different process states
    let state1 = create_state(&[("x", 1), ("y", 2)], &[("P_0.pc", 1), ("P_0.local", 5)]);
    let state2 = create_state(&[("x", 1), ("y", 2)], &[("P_0.pc", 2), ("P_0.local", 5)]);
    
    assert!(store.insert(&state1));
    assert!(store.insert(&state2)); // Different P_0 state
    
    // Same globals should reuse ordinal
    assert_eq!(store.component_maps[0].len(), 1);
    // Different process states should have different ordinals
    assert_eq!(store.component_maps[1].len(), 2);
}

#[test]
fn test_collapse_deduplication() {
    let metadata = vec![/* ... */];
    let mut store = CollapseStore::new(metadata);
    
    // Insert same state twice
    let state = create_state(/* ... */);
    assert!(store.insert(&state)); // First insert: true
    assert!(!store.insert(&state)); // Second insert: false (duplicate)
    
    assert_eq!(store.len(), 1);
}

#[test]
fn test_collapse_compression_ratio() {
    // Create store with known state space
    let mut store = CollapseStore::new(/* ... */);
    
    // Insert 1000 states
    for i in 0..1000 {
        let state = create_state_with_value(i);
        store.insert(&state);
    }
    
    let ratio = store.compression_ratio();
    assert!(ratio > 1.0); // Some compression achieved
}
```

### Integration Tests

```rust
#[test]
fn test_collapse_with_model_checker() {
    let promela = include_str!("../examples/peterson.pml");
    let model = LuaModel::from_source(promela).unwrap();
    
    // Run with collapse mode
    let checker_collapse = CheckerBuilder::new()
        .model(model.clone())
        .storage_mode(StorageMode::Collapse { metadata: get_metadata() })
        .build();
    let result_collapse = checker_collapse.check();
    
    // Run with exact mode
    let checker_exact = CheckerBuilder::new()
        .model(model)
        .storage_mode(StorageMode::Exact)
        .build();
    let result_exact = checker_exact.check();
    
    // Same verification result
    assert_eq!(result_collapse.errors, result_exact.errors);
    // Collapse should use less memory (fewer states stored due to dedup)
    assert!(result_collapse.states_stored <= result_exact.states_stored);
}
```

### Benchmark

```rust
#[bench]
fn bench_collapse_insert(b: &mut Bencher) {
    let mut store = CollapseStore::new(/* ... */);
    let state = create_typical_state();
    
    b.iter(|| {
        store.insert(&state);
    });
}

#[bench]
fn bench_collapse_vs_exact(b: &mut Bencher) {
    // Compare collapse vs exact insertion performance
    // ...
}
```

## Dependencies

- **DashMap** (new dependency): concurrent hash map for component canonicalization
- Existing `StorageMode` enum (enhanced)
- Codegen metadata generation (new)

## Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Component extraction slow | Performance degradation | Cache variable offsets; direct memory access |
| Poor compression on some models | Memory savings not realized | Fall back to exact mode; document limitations |
| Metadata generation complex | Implementation delay | Start with simple per-process grouping; enhance later |
| Collision in canonicalization | Unsound verification | Thorough testing; property-based validation |

## Success Criteria

- ✅ Compression ratio ≥ 5x for models with 10+ processes
- ✅ Insertion overhead < 10μs per state
- ✅ Same verification results as exact mode
- ✅ Memory usage within 2x of Spin's collapse mode
- ✅ Transparent integration (no API changes for users)
