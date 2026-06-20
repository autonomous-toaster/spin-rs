# Nested DFS Specification

## Overview

Implement nested depth-first search for detecting accepting cycles in the product space (model × Büchi), enabling LTL liveness verification.

## Requirements

### Functional Requirements

### Requirement: Outer DFS

The nested DFS implementation SHALL ALWAYS implement the outer DFS as follows:

- **Product space exploration**: The outer DFS MUST ALWAYS explore the product space from the initial product state. [R1.1]
- **Visited tracking**: The outer DFS MUST ALWAYS track visited states in a `visited1` set. [R1.2]
- **Accepting state detection**: IF the outer DFS reaches an accepting state, THEN it MUST initiate the inner DFS. [R1.3]
- **Path maintenance**: The outer DFS MUST ALWAYS maintain a current path (stack) for cycle detection. [R1.4]
- **Trail recording**: The outer DFS MUST ALWAYS record transition labels for the error trail. [R1.5]

### Requirement: Inner DFS

The nested DFS implementation SHALL ALWAYS implement the inner DFS as follows:

- **Cycle search**: The inner DFS MUST search for a cycle back to the accepting state that triggered the inner DFS. [R2.1]
- **Visited tracking**: The inner DFS MUST ALWAYS track visited states in a `visited2` set. [R2.2]
- **Cycle detection**: IF the inner DFS reaches a state already in `visited2`, THEN it MUST report a cycle as detected. [R2.3]
- **Error trail**: The inner DFS MUST return an error trail when a cycle is found. [R2.4]

### Requirement: Violation Reporting

The nested DFS implementation SHALL ALWAYS report violations as follows:

- **Violation structure**: The implementation MUST ALWAYS construct a `Violation` with property name, description, and trail. [R3.1]
- **Trail content**: The trail SHALL ALWAYS include transition labels from the initial state to the cycle. [R3.2]
- **Multiple violations**: The implementation SHOULD support collecting multiple violations (return first, optionally collect more). [R3.3]

### Requirement: Termination

The nested DFS implementation SHALL ALWAYS terminate as follows:

- **Complete exploration**: IF all reachable states have been explored, THEN the DFS MUST terminate (no violations). [R4.1]
- **Early termination**: IF an accepting cycle is found, THEN the DFS MUST terminate immediately (violation detected). [R4.2]
- **Depth limit**: The DFS MUST support a max depth limit to prevent infinite search on large models. [R4.3]

### Requirement: Performance

The nested DFS implementation SHALL ALWAYS meet the following performance requirements:

- **Search speed**: Outer DFS SHALL explore states at a rate of >10k states/second. [R5.1]
- **Overhead**: Inner DFS overhead SHALL be <10% of total search time. [R5.2]
- **Memory**: Memory usage SHALL be O(depth) for the stack plus O(states) for visited sets. [R5.3]

### Requirement: Correctness

The nested DFS implementation SHALL ALWAYS satisfy the following correctness requirements:

- **Completeness**: IF accepting cycles exist, THEN Nested DFS SHALL find them. [R6.1]
- **Soundness**: Nested DFS SHALL NOT report spurious cycles. [R6.2]
- **Trail validity**: Error trails SHALL ALWAYS be valid paths from the initial state to the cycle. [R6.3]

## Interface

```rust
/// Nested DFS for LTL verification
pub struct NestedDFS<S> {
    visited1: HashSet<ProductState<S>>,  // Outer DFS visited
    visited2: HashSet<ProductState<S>>,  // Inner DFS visited
    stack: Vec<ProductState<S>>,         // Current path
    trail: Vec<String>,                  // Transition labels
}

impl<S: Clone + Hash + Eq + Send> NestedDFS<S> {
    pub fn new() -> Self;
    
    /// Run nested DFS, return violation if found
    pub fn check<M>(
        &mut self,
        model: &M,
        buchi: &BuchiAutomaton,
        init_product: ProductState<S>,
    ) -> Option<Violation>
    where
        M: Model<State = S>;
}

/// Violation with error trail
pub struct Violation {
    pub property_name: String,
    pub trail: Vec<String>,
    pub description: String,
}
```

## Algorithm

### Outer DFS (dfs1)

```
dfs1(state):
  visited1.add(state)
  stack.push(state)
  
  for each product_transition from state:
    if transition.next not in visited1:
      trail.push(transition.label)
      if violation = dfs1(transition.next):
        return violation
      trail.pop()
    else if transition.is_accepting:
      # Reached accepting state that's already visited
      # Start inner DFS to check for cycle
      if violation = dfs2(transition.next):
        return violation
  
  stack.pop()
  return None
```

### Inner DFS (dfs2)

```
dfs2(state):
  if state in visited2:
    # Cycle detected!
    return Violation {
      property_name: current_property,
      trail: trail.clone(),
      description: "Accepting cycle detected"
    }
  
  visited2.add(state)
  
  for each product_transition from state:
    if transition.next not in visited2:
      if violation = dfs2(transition.next):
        return violation
  
  return None
```

## Examples

### Liveness Violation

```rust
let promela = r#"
    byte x = 0;
    active proctype P() {
        x = 1;  // x becomes 1 and stays 1
    }
"#;
let ltl = "[]<>(x == 0)";  // Always eventually x=0 (violated!)

let model = LuaModel::from_source(promela).unwrap();
let formula = parse_ltl(ltl).unwrap();
let buchi = to_buchi(&formula).unwrap();

let init_state = model.init_states()[0].clone();
let init_product = ProductState::new(
    init_state,
    buchi.initial,
    model.hash(&init_state),
);

let mut dfs = NestedDFS::new();
let violation = dfs.check(&model, &buchi, init_product);

assert!(violation.is_some());
assert_eq!(violation.unwrap().property_name, "liveness");
```

### Liveness Holds

```rust
let promela = r#"
    byte x = 0;
    active proctype P() {
        do
        :: x = 0
        :: x = 1
        od
    }
"#;
let ltl = "[]<>(x == 0)";  // Always eventually x=0 (holds!)

// ... same setup as above ...

let violation = dfs.check(&model, &buchi, init_product);
assert!(violation.is_none());  // No cycle found, property holds
```

## Testing

### Unit Tests

- Outer DFS visits all reachable states
- Inner DFS detects cycles correctly
- Violation construction with correct trail
- Termination on max depth

### Integration Tests

- Liveness violations on simple models (x=1 forever)
- Liveness holds on oscillating models (x=0, x=1, x=0, ...)
- Compare with Spin on standard examples (Peterson, leader election)

### Property Tests

- Generated models + generated LTL → verify nested DFS finds violations when they exist
- Trail validity (each step is a valid transition)

## Dependencies

- `product-construction` spec (for `ProductState`, `ProductTransition`)
- `buchi-construction` spec (for `BuchiAutomaton`)

## Success Criteria

- ✅ Nested DFS finds accepting cycles when they exist
- ✅ Nested DFS terminates with no violations when no cycles exist
- ✅ Error trails are valid and actionable
- ✅ Performance meets R5 requirements
- ✅ Correctness matches Spin on standard examples
