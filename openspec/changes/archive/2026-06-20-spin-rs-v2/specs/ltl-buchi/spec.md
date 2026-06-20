# LTL → Büchi Verification

## Overview

Implement full LTL verification using the omega-automata crate to convert LTL formulas to Büchi automata, then perform nested DFS on the product space (model state × Büchi state) to detect accepting cycles.

## Requirements

### Functional Requirements

#### R1: LTL Formula Parsing

- **R1.1**: Parse all standard LTL operators: `[]` (always), `<>` (eventually), `X` (next), `U` (until), `V` (release)
- **R1.2**: Parse boolean operators: `&&`, `||`, `->`, `!`
- **R1.3**: Parse atomic propositions (variable comparisons, process states)
- **R1.4**: Support nested formulas of arbitrary depth
- **R1.5**: Support Spin syntax: `[]<>(p -> <>q)`, `(p U q) V r`, etc.

#### R2: LTL → Büchi Conversion

- **R2.1**: Convert parsed LTL to omega-automata `Formulas` representation
- **R2.2**: Apply Vardi-Wolper tableau algorithm via `LtlToVWABW`
- **R2.3**: Convert VWABW → GBW via `VWABWToGBW`
- **R2.4**: Convert GBW → NBW via `GBWToNBW`
- **R2.5**: Extract Büchi automaton structure (states, transitions, accepting states)
- **R2.6**: Handle all LTL operators correctly (no simplifications that lose expressiveness)

#### R3: Product Construction

- **R3.1**: Construct synchronous product of model state space and Büchi automaton
- **R3.2**: Synchronize on atomic propositions (model state determines which Büchi transitions are enabled)
- **R3.3**: Track accepting states in product space (product state is accepting iff Büchi component is accepting)
- **R3.4**: Support on-the-fly product construction (don't precompute full product)

#### R4: Nested DFS

- **R4.1**: Implement outer DFS to reach accepting states
- **R4.2**: Implement inner DFS to detect cycles from accepting states
- **R4.3**: Return error trail when accepting cycle is found
- **R4.4**: Support multiple accepting states (generalized Büchi not required, but handle multiple accepting states in NBW)

#### R5: Integration

- **R5.1**: Integrate with existing `Model` trait (product construction wraps underlying model)
- **R5.2**: Support multiple LTL properties per verification run
- **R5.3**: Support inline LTL (`ltl p0 { ... }`) and command-line LTL (`--ltl name 'formula'`)
- **R5.4**: Report liveness violations separately from safety violations

### Non-Functional Requirements

#### R6: Performance

- **R6.1**: Büchi construction overhead < 100ms for typical formulas (<10 operators)
- **R6.2**: Product state hashing < 1μs (cached hash)
- **R6.3**: Büchi transition matching < 10μs per model transition
- **R6.4**: Memory overhead for Büchi automaton < 1MB for typical formulas

#### R7: Correctness

- **R7.1**: Never miss an accepting cycle (completeness)
- **R7.2**: Never report a spurious accepting cycle (soundness)
- **R7.3**: Büchi automaton must be equivalent to the original LTL formula
- **R7.4**: Product construction must correctly synchronize model and Büchi transitions

#### R8: Usability

- **R8.1**: Error messages reference LTL formula, not just "liveness violation"
- **R8.2**: Error trail shows both model transitions and Büchi state changes
- **R8.3**: Support `--ltl` CLI flag with intuitive syntax

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                    LTL Verification Pipeline                         │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  LTL Formula (String)                                               │
│  "[]<>(x == 0)"                                                     │
│      │                                                              │
│      ▼                                                              │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ LtlFormula::parse()                                         │   │
│  │ - Recursive descent parser                                  │   │
│  │ - Produces LtlFormula AST                                   │   │
│  └─────────────────────────────────────────────────────────────┘   │
│      │                                                              │
│      ▼                                                              │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ omega-automata conversion                                   │   │
│  │                                                             │   │
│  │  LtlFormula → omega_ltl::Formula                           │   │
│  │       │                                                     │   │
│  │       ▼                                                     │   │
│  │  LtlToVWABW (Very Weak Alternating Büchi)                  │   │
│  │       │                                                     │   │
│  │       ▼                                                     │   │
│  │  VWABWToGBW (Generalized Büchi)                            │   │
│  │       │                                                     │   │
│  │       ▼                                                     │   │
│  │  GBWToNBW (Non-deterministic Büchi)                        │   │
│  └─────────────────────────────────────────────────────────────┘   │
│      │                                                              │
│      ▼                                                              │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ BuchiAutomaton                                              │   │
│  │ - states: Vec<BuchiState>                                   │   │
│  │ - initial: usize                                            │   │
│  │ - accepting: HashSet<usize>                                 │   │
│  │ - transitions: Vec<Vec<BuchiTransition>>                    │   │
│  └─────────────────────────────────────────────────────────────┘   │
│      │                                                              │
│      ▼                                                              │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ ProductConstruction                                         │   │
│  │                                                             │   │
│  │  For each model transition:                                 │   │
│  │  1. Evaluate atomic props in next state                    │   │
│  │  2. Find matching Büchi transitions                        │   │
│  │  3. Create product transitions                              │   │
│  │                                                             │   │
│  │  ProductState {                                             │   │
│  │    model_state: S,                                          │   │
│  │    buchi_state: usize,                                      │   │
│  │  }                                                          │   │
│  └─────────────────────────────────────────────────────────────┘   │
│      │                                                              │
│      ▼                                                              │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ NestedDFS                                                   │   │
│  │                                                             │   │
│  │  Outer DFS:                                                 │   │
│  │  - Explore product space                                    │   │
│  │  - Track visited: HashSet<ProductState>                     │   │
│  │  - When reaching accepting state: start inner DFS          │   │
│  │                                                             │   │
│  │  Inner DFS:                                                 │   │
│  │  - Search for cycle back to accepting state                │   │
│  │  - Track visited: HashSet<ProductState>                     │   │
│  │  - If cycle found: liveness violation                      │   │
│  └─────────────────────────────────────────────────────────────┘   │
│      │                                                              │
│      ▼                                                              │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ Violation                                                   │   │
│  │ - property_name: "p0"                                       │   │
│  │ - description: "LTL []<>(x == 0) violated"                  │   │
│  │ - trail: Vec<String> (model + Büchi transitions)           │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Data Structures

### LTL Formula (Enhanced)

```rust
/// LTL formula with omega-automata integration
#[derive(Debug, Clone)]
pub enum LtlFormula {
    True,
    False,
    Atom(String),
    Not(Box<LtlFormula>),
    And(Box<LtlFormula>, Box<LtlFormula>),
    Or(Box<LtlFormula>, Box<LtlFormula>),
    Implies(Box<LtlFormula>, Box<LtlFormula>),
    Always(Box<LtlFormula>),
    Eventually(Box<LtlFormula>),
    Next(Box<LtlFormula>),
    Until(Box<LtlFormula>, Box<LtlFormula>),
    Release(Box<LtlFormula>, Box<LtlFormula>),
}

impl LtlFormula {
    /// Convert to omega-automata representation
    pub fn to_omega(&self, formulas: &mut omega_automata::ltl::Formulas) -> omega_automata::ltl::Formula {
        match self {
            LtlFormula::True => formulas.constant(true),
            LtlFormula::False => formulas.constant(false),
            LtlFormula::Atom(name) => {
                // Map atomic prop name to atom ID
                let id = self.atom_name_to_id(name);
                formulas.atom(id)
            }
            LtlFormula::Not(f) => formulas.neg(f.to_omega(formulas)),
            LtlFormula::And(f1, f2) => {
                let i1 = f1.to_omega(formulas);
                let i2 = f2.to_omega(formulas);
                formulas.and(i1, i2)
            }
            // ... etc for other operators
        }
    }
}
```

### Büchi Automaton

```rust
/// Büchi automaton for LTL verification
pub struct BuchiAutomaton {
    /// Number of states
    pub num_states: usize,
    /// Initial state index
    pub initial: usize,
    /// Accepting state indices
    pub accepting: HashSet<usize>,
    /// Transitions per state
    pub transitions: Vec<Vec<BuchiTransition>>,
}

pub struct BuchiTransition {
    /// Target state
    pub to: usize,
    /// Conditions: (atomic_prop_name, must_be_true)
    pub conditions: Vec<(String, bool)>,
}

impl BuchiAutomaton {
    /// Construct from LTL formula using omega-automata
    pub fn from_ltl(formula: &LtlFormula) -> anyhow::Result<Self> {
        use omega_automata::ltl::*;
        use omega_automata::automata::{abw::*, gbw::*, nbw::*};
        
        let mut formulas = Formulas::default();
        let omega_formula = formula.to_omega(&mut formulas);
        let normalized = formulas.normalize(omega_formula);
        
        // LTL → VWABW → GBW → NBW
        let vwabw = ltl_to_abw(formulas.access(normalized));
        let gbw = vwabw_to_gbw(&vwabw);
        let nbw = gbw_to_nbw(&gbw);
        
        // Extract NBW structure
        Ok(Self {
            num_states: nbw.num_states(),
            initial: nbw.initial_state(),
            accepting: nbw.accepting_states().collect(),
            transitions: Self::extract_transitions(&nbw),
        })
    }
    
    fn extract_transitions(nbw: &Nbw) -> Vec<Vec<BuchiTransition>> {
        // Extract transition structure from omega-automata NBW
        // This requires iterating over NBW states and transitions
        // and converting to our BuchiTransition format
        // ...
    }
}
```

### Product State

```rust
/// Product state for nested DFS
#[derive(Clone, Debug)]
pub struct ProductState<S> {
    /// Model state
    pub model_state: S,
    /// Büchi state
    pub buchi_state: usize,
    /// Cached hash for performance
    cached_hash: u64,
}

impl<S: Hash> Hash for ProductState<S> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Use cached hash directly
        state.write_u64(self.cached_hash);
    }
}

impl<S: Eq> Eq for ProductState<S> {}

impl<S: PartialEq> PartialEq for ProductState<S> {
    fn eq(&self, other: &Self) -> bool {
        self.model_state == other.model_state
            && self.buchi_state == other.buchi_state
    }
}

impl<S> ProductState<S> {
    pub fn new(model_state: S, buchi_state: usize, model_hash: u64) -> Self {
        // Compute cached hash: hash(model_state, buchi_state)
        let cached_hash = Self::compute_hash(model_hash, buchi_state);
        Self {
            model_state,
            buchi_state,
            cached_hash,
        }
    }
    
    fn compute_hash(model_hash: u64, buchi_state: usize) -> u64 {
        let mut hasher = FxHasher::default();
        model_hash.hash(&mut hasher);
        buchi_state.hash(&mut hasher);
        hasher.finish()
    }
}
```

### Product Transition

```rust
pub struct ProductTransition<S> {
    /// Transition label (from model transition)
    pub label: String,
    /// Next product state
    pub next: ProductState<S>,
    /// Whether this transition visits an accepting Büchi state
    pub is_accepting: bool,
}

/// Synchronize model and Büchi transitions
pub fn sync_transitions<S, M: Model<State = S>>(
    model: &M,
    state: &S,
    model_transitions: &[Transition<S>],
    buchi: &BuchiAutomaton,
    buchi_state: usize,
) -> Vec<ProductTransition<S>> {
    let mut product_transitions = Vec::new();
    
    for model_trans in model_transitions {
        // Evaluate atomic propositions in next state
        let atomic_props = evaluate_atomic_props(model, &model_trans.next);
        
        // Find enabled Büchi transitions
        for buchi_trans in &buchi.transitions[buchi_state] {
            if buchi_transition_enabled(buchi_trans, &atomic_props) {
                let next_product = ProductState::new(
                    model_trans.next.clone(),
                    buchi_trans.to,
                    model.hash(&model_trans.next),
                );
                
                product_transitions.push(ProductTransition {
                    label: model_trans.label.clone(),
                    next: next_product,
                    is_accepting: buchi.accepting.contains(&buchi_trans.to),
                });
            }
        }
    }
    
    product_transitions
}

fn evaluate_atomic_props<S, M: Model<State = S>>(
    model: &M,
    state: &S,
) -> HashSet<String> {
    // Extract atomic propositions from state
    // This requires model-specific logic or metadata from codegen
    // For v2, we'll use a simple heuristic: check all boolean variables
    // Full implementation would use codegen metadata
    HashSet::new()
}

fn buchi_transition_enabled(
    trans: &BuchiTransition,
    atomic_props: &HashSet<String>,
) -> bool {
    trans.conditions.iter().all(|(prop, must_be_true)| {
        atomic_props.contains(prop) == *must_be_true
    })
}
```

## Nested DFS Algorithm

```rust
pub struct NestedDFS<S> {
    /// Outer DFS visited set
    visited1: HashSet<ProductState<S>>,
    /// Inner DFS visited set
    visited2: HashSet<ProductState<S>>,
    /// Current path (for cycle detection)
    stack: Vec<ProductState<S>>,
    /// Error trail
    trail: Vec<String>,
}

impl<S: Clone + Hash + Eq + Send> NestedDFS<S> {
    pub fn new() -> Self {
        Self {
            visited1: HashSet::new(),
            visited2: HashSet::new(),
            stack: Vec::new(),
            trail: Vec::new(),
        }
    }
    
    /// Run nested DFS from initial product state
    pub fn check<M>(
        &mut self,
        model: &M,
        buchi: &BuchiAutomaton,
        init_product: ProductState<S>,
    ) -> Option<Violation>
    where
        M: Model<State = S>,
    {
        // Outer DFS
        if let Some(violation) = self.dfs1(model, buchi, init_product) {
            return Some(violation);
        }
        None
    }
    
    fn dfs1<M>(
        &mut self,
        model: &M,
        buchi: &BuchiAutomaton,
        state: ProductState<S>,
    ) -> Option<Violation>
    where
        M: Model<State = S>,
    {
        self.visited1.insert(state.clone());
        self.stack.push(state.clone());
        
        // Get product transitions
        let model_state = &state.model_state;
        let model_transitions = model.transitions(model_state);
        let product_transitions = sync_transitions(
            model,
            model_state,
            &model_transitions,
            buchi,
            state.buchi_state,
        );
        
        for trans in product_transitions {
            if !self.visited1.contains(&trans.next) {
                self.trail.push(trans.label.clone());
                if let Some(violation) = self.dfs1(model, buchi, trans.next) {
                    return Some(violation);
                }
                self.trail.pop();
            } else if trans.is_accepting {
                // Reached an accepting state that's already visited
                // Start inner DFS to check for cycle
                if let Some(violation) = self.dfs2(model, buchi, trans.next.clone()) {
                    return Some(violation);
                }
            }
        }
        
        self.stack.pop();
        None
    }
    
    fn dfs2<M>(
        &mut self,
        model: &M,
        buchi: &BuchiAutomaton,
        state: ProductState<S>,
    ) -> Option<Violation>
    where
        M: Model<State = S>,
    {
        if self.visited2.contains(&state) {
            // Found a cycle! Construct violation
            return Some(Violation {
                property_name: "liveness".to_string(),
                trail: self.trail.clone(),
                description: "Accepting cycle detected".to_string(),
            });
        }
        
        self.visited2.insert(state.clone());
        
        // Get product transitions
        let model_state = &state.model_state;
        let model_transitions = model.transitions(model_state);
        let product_transitions = sync_transitions(
            model,
            model_state,
            &model_transitions,
            buchi,
            state.buchi_state,
        );
        
        for trans in product_transitions {
            if !self.visited2.contains(&trans.next) {
                if let Some(violation) = self.dfs2(model, buchi, trans.next) {
                    return Some(violation);
                }
            }
        }
        
        None
    }
}
```

## Interface

### Library API

```rust
/// Verify an LTL property on a Promela model
pub fn verify_ltl(
    promela: &str,
    formula: &str,
    property_name: &str,
) -> anyhow::Result<Option<Violation>> {
    // Parse Promela
    let model_ast = parser::parse(promela)?;
    
    // Parse LTL
    let ltl_formula = LtlFormula::parse(formula)?;
    
    // Generate Lua
    let lua = codegen::generate(&model_ast);
    
    // Create runtime model
    let model = runtime::LuaModel::from_source(&lua.source)?;
    
    // Construct Büchi automaton
    let buchi = BuchiAutomaton::from_ltl(&ltl_formula)?;
    
    // Create initial product state
    let init_states = model.init_states();
    let init_product = ProductState::new(
        init_states[0].clone(),
        buchi.initial,
        model.hash(&init_states[0]),
    );
    
    // Run nested DFS
    let mut dfs = NestedDFS::new();
    let violation = dfs.check(&model, &buchi, init_product);
    
    Ok(violation.map(|mut v| {
        v.property_name = property_name.to_string();
        v
    }))
}
```

### CLI

```bash
# Verify LTL property
spin-rs --ltl p0 '[]<>(x == 0)' model.pml

# Verify LTL from file
spin-rs --ltl-file properties.ltl model.pml

# Multiple properties
spin-rs --ltl p0 '[]<>(x == 0)' --ltl p1 '[](y -> <>z)' model.pml
```

## Testing

### Unit Tests

```rust
#[test]
fn test_ltl_parsing() {
    assert!(matches!(
        LtlFormula::parse("[]p"),
        Ok(LtlFormula::Always(_))
    ));
    assert!(matches!(
        LtlFormula::parse("<>p"),
        Ok(LtlFormula::Eventually(_))
    ));
    assert!(matches!(
        LtlFormula::parse("p U q"),
        Ok(LtlFormula::Until(_, _))
    ));
}

#[test]
fn test_buchi_construction() {
    let formula = LtlFormula::parse("[]<>p").unwrap();
    let buchi = BuchiAutomaton::from_ltl(&formula).unwrap();
    
    assert!(buchi.num_states > 0);
    assert!(!buchi.accepting.is_empty());
}

#[test]
fn test_product_construction() {
    // Simple model with 2 states
    let model = SimpleModel::new();
    let formula = LtlFormula::parse("[]p").unwrap();
    let buchi = BuchiAutomaton::from_ltl(&formula).unwrap();
    
    // Construct product, verify size
    // ...
}
```

### Integration Tests

```rust
#[test]
fn test_peterson_liveness() {
    let promela = include_str!("../examples/peterson.pml");
    let ltl = "[]!(crit0 && crit1)"; // Mutual exclusion
    
    let result = verify_ltl(promela, ltl, "mutual_exclusion").unwrap();
    assert!(result.is_none()); // Should hold
}

#[test]
fn test_liveness_violation() {
    let promela = r#"
        byte x = 0;
        active proctype P() {
            x = 1; // x becomes 1 and stays 1
        }
    "#;
    let ltl = "[]<>(x == 0)"; // Always eventually x=0
    
    let result = verify_ltl(promela, ltl, "liveness").unwrap();
    assert!(result.is_some()); // Should be violated
}
```

## Dependencies

- **omega-automata** (already in Cargo.toml): LTL → Büchi conversion
- **FxHash** (already in Cargo.toml): Fast state hashing

## Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| omega-automata API changes | Blocked on dependency | Pin version; wrap in abstraction layer |
| Product space explosion | Memory exhaustion | On-the-fly construction; bitstate mode for product |
| Büchi transition matching slow | Performance degradation | Precompute bitmasks; optimize hot path |
| Atomic prop evaluation complex | Implementation delay | Start with simple heuristics; enhance later |

## Success Criteria

- ✅ Parse and verify all standard LTL formulas
- ✅ Detect liveness violations that v1 misses
- ✅ No false positives (soundness)
- ✅ No false negatives (completeness)
- ✅ Performance within 2x of v1 for safety properties
- ✅ Performance within 5x of Spin 6.5.x for LTL properties
