# Product Construction Specification

## Overview

Construct the synchronous product of a model state space and a Büchi automaton for LTL verification.

## Requirements

### Functional Requirements

### Requirement: Product State Construction

The product construction SHALL ALWAYS implement product states as follows:

- **Product state representation**: Product states MUST ALWAYS be represented as `(model_state, buchi_state)` pairs. [R1.1]
- **Cached hash**: The implementation MUST ALWAYS compute a cached hash for product states by combining the model hash and Büchi state. [R1.2]
- **Equality and hashing**: The implementation MUST ALWAYS implement `Eq`, `PartialEq`, and `Hash` traits for product states. [R1.3]
- **Generic support**: The implementation MUST ALWAYS support generic model state type `S`. [R1.4]

### Requirement: Transition Synchronization

The product construction SHALL ALWAYS synchronize transitions as follows:

- **Atomic proposition evaluation**: For each model transition, the implementation MUST evaluate atomic propositions in the next state. [R2.1]
- **Büchi matching**: The implementation MUST match model transitions with enabled Büchi transitions based on atomic proposition values. [R2.2]
- **Product transitions**: The implementation MUST create product transitions containing `(model_label, next_product_state, is_accepting)`. [R2.3]
- **Multiple enabled transitions**: The implementation MUST handle multiple enabled Büchi transitions per model transition. [R2.4]

### Requirement: Atomic Proposition Evaluation

The product construction SHALL ALWAYS evaluate atomic propositions as follows:

- **Variable extraction**: The implementation MUST ALWAYS extract boolean variable values from the model state. [R3.1]
- **Condition evaluation**: The implementation MUST ALWAYS evaluate atomic proposition conditions such as `x == 0` and `flag`. [R3.2]
- **Comparison operators**: The implementation MUST ALWAYS support variable comparisons: `==`, `!=`, `<`, `>`, `<=`, `>=`. [R3.3]
- **Undefined variables**: IF a variable is undefined, THEN the implementation MUST return false. [R3.4]

### Requirement: Performance

The product construction SHALL ALWAYS meet the following performance requirements:

- **Hashing speed**: Product state hashing SHALL be <1μs (cached). [R4.1]
- **Evaluation speed**: Atomic proposition evaluation SHALL be <10μs per state. [R4.2]
- **Memory**: The implementation SHOULD minimize allocation during transition synchronization. [R4.3]

### Requirement: Correctness

The product construction SHALL ALWAYS satisfy the following correctness requirements:

- **Synchronization**: Product transitions SHALL ALWAYS correctly synchronize model and Büchi components. [R5.1]
- **Accepting states**: Accepting product states SHALL ALWAYS be identified correctly based on whether the Büchi component is accepting. [R5.2]
- **Completeness**: All enabled transitions SHALL ALWAYS be enumerated. [R5.3]

## Interface

```rust
/// Product state for nested DFS
pub struct ProductState<S> {
    pub model_state: S,
    pub buchi_state: usize,
    cached_hash: u64,
}

/// Product transition
pub struct ProductTransition<S> {
    pub label: String,
    pub next: ProductState<S>,
    pub is_accepting: bool,
}

/// Synchronize model and Büchi transitions
pub fn sync_transitions<S, M>(
    model: &M,
    state: &S,
    model_transitions: &[Transition<S>],
    buchi: &BuchiAutomaton,
    buchi_state: usize,
) -> Vec<ProductTransition<S>>
where
    M: Model<State = S>;

/// Evaluate atomic propositions in a model state
pub fn evaluate_atomic_props<S, M>(
    model: &M,
    state: &S,
) -> HashMap<String, bool>;
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                Product Construction                          │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Model State (S)           Büchi Automaton                  │
│  ┌──────────────┐           ┌──────────────┐               │
│  │  x = 0       │           │   s0 (init)  │               │
│  │  y = 1       │           │   accepting  │               │
│  │  flag = true │           └──────┬───────┘               │
│  └──────┬───────┘                  │                        │
│         │                          │                        │
│         │                          │ 1. Get Büchi trans     │
│         │◄─────────────────────────┘    from s0             │
│         │                                                    │
│         │ 2. Evaluate atomic props                          │
│         │    { "x==0": true, "y==1": true, "flag": true }  │
│         │                                                    │
│         │ 3. Match Büchi transitions                        │
│         │    s0 --x==0--> s1 (enabled)                      │
│         │    s0 --y!=1--> s2 (disabled)                     │
│         │                                                    │
│         │ 4. Create product transitions                     │
│         ▼                                                    │
│  ┌──────────────────────────────────────────────┐          │
│  │  ProductTransition {                         │          │
│  │    label: "P:x=1",                           │          │
│  │    next: ProductState { model=x=1, buchi=s1 },          │
│  │    is_accepting: false                       │          │
│  │  }                                           │          │
│  └──────────────────────────────────────────────┘          │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## Examples

### Simple Product Construction

```rust
let model = MyModel::new();
let buchi = to_buchi(&parse_ltl("[](x == 0)").unwrap()).unwrap();
let init_state = model.init_states()[0].clone();

let init_product = ProductState::new(
    init_state,
    buchi.initial,
    model.hash(&init_state),
);

let model_transitions = model.transitions(&init_product.model_state);
let product_transitions = sync_transitions(
    &model,
    &init_product.model_state,
    &model_transitions,
    &buchi,
    init_product.buchi_state,
);

// product_transitions now contains all enabled transitions
```

### Atomic Proposition Evaluation

```rust
let props = evaluate_atomic_props(&model, &state);
assert_eq!(props.get("x == 0"), Some(&true));
assert_eq!(props.get("y == 1"), Some(&true));
assert_eq!(props.get("flag"), Some(&true));
```

## Testing

### Unit Tests

- Product state hash computation
- Atomic proposition evaluation (true/false cases)
- Transition synchronization (enabled/disabled cases)
- Accepting state identification

### Integration Tests

- Full product construction on simple models
- Verify product size = model_size × buchi_size (for full exploration)
- Compare with manual product construction

### Property Tests

- Generated models × generated Büchi → verify product structure
- Transition coverage (all enabled transitions present)

## Dependencies

- `buchi-construction` spec (for `BuchiAutomaton`)
- spin-rs `Model` trait (for model interface)

## Success Criteria

- ✅ Product states correctly combine model and Büchi components
- ✅ Transition synchronization correctly matches enabled Büchi transitions
- ✅ Atomic proposition evaluation is accurate
- ✅ Performance meets R4 requirements
- ✅ Product construction enables correct nested DFS
