# Büchi Construction Specification

## Overview

Construct Büchi automata from LTL formulas using pattern-based construction for supported operators.

## Requirements

### Functional Requirements

### Requirement: Pattern Recognition

The Büchi constructor SHALL ALWAYS recognize the following patterns:

- **Always pattern**: The constructor MUST ALWAYS recognize `[]p` (always) patterns where `p` is an atomic proposition. [R1.1]
- **Eventually pattern**: The constructor MUST ALWAYS recognize `<>p` (eventually) patterns where `p` is an atomic proposition. [R1.2]
- **Next pattern**: The constructor MUST ALWAYS recognize `Xp` (next) patterns where `p` is an atomic proposition. [R1.3]
- **Negation pattern**: The constructor MUST ALWAYS recognize `!p` (negation) patterns where `p` is an atomic proposition. [R1.4]
- **Conjunction pattern**: The constructor MUST ALWAYS recognize `p && q` (conjunction) patterns where `p` and `q` are atomic propositions. [R1.5]
- **Disjunction pattern**: The constructor MUST ALWAYS recognize `p || q` (disjunction) patterns where `p` and `q` are atomic propositions. [R1.6]

### Requirement: Automaton Construction

The constructor SHALL ALWAYS build automata as follows:

- **Always automaton**: The constructor MUST construct a 2-state automaton for `[]p` with an accepting loop on `p` and a rejecting sink on `!p`. [R2.1]
- **Eventually automaton**: The constructor MUST construct a 2-state automaton for `<>p` with an accepting state reachable on `p` and a self-loop on `!p`. [R2.2]
- **Next automaton**: The constructor MUST construct a 2-3 state automaton for `Xp` with a transition to a state that checks `p`. [R2.3]
- **Conjunction automaton**: The constructor MUST construct a product automaton for `p && q` using cross-product of component automata with intersection of accepting sets. [R2.4]
- **Disjunction automaton**: The constructor MUST construct a product automaton for `p || q` using cross-product of component automata with union of accepting sets. [R2.5]
- **Negation automaton**: The constructor MUST construct a complement automaton for `!p` by swapping accepting and rejecting states. [R2.6]

### Requirement: Error Handling

The constructor SHALL ALWAYS handle errors as follows:

- **Nested temporal**: IF the input formula is `[]<>p` or `<>(p && q)` where the subformula is temporal, THEN the constructor MUST return `LtlError::NestedTemporal`. [R3.1]
- **Unrecognized patterns**: IF the input contains unrecognized patterns, THEN the constructor MUST return `LtlError::UnsupportedOperator`. [R3.2]
- **Validation**: The constructor MUST ALWAYS validate that atomic propositions are well-formed before construction. [R3.3]

### Requirement: Performance

The constructor SHALL ALWAYS meet the following performance requirements:

- **Construction time**: The constructor MUST construct automata for simple formulas (<5 operators) in <50μs. [R4.1]
- **Automaton size**: Automata for simple formulas SHALL ALWAYS have ≤4 states. [R4.2]
- **Memory**: The constructor SHOULD minimize memory allocation during construction. [R4.3]

### Requirement: Correctness

The constructor SHALL ALWAYS satisfy the following correctness requirements:

- **Language equivalence**: Constructed automata SHALL ALWAYS accept exactly the words satisfying the formula. [R5.1]
- **Accepting states**: Accepting states SHALL ALWAYS be correctly identified for each pattern. [R5.2]
- **Transition coverage**: Transitions SHALL ALWAYS cover all possible atomic proposition valuations. [R5.3]

## Interface

```rust
/// Construct Büchi automaton from LTL formula
pub fn to_buchi(formula: &LtlFormula) -> Result<BuchiAutomaton, LtlError>;

/// Büchi automaton structure
pub struct BuchiAutomaton {
    pub num_states: usize,
    pub initial: usize,
    pub accepting: HashSet<usize>,
    pub transitions: Vec<Vec<BuchiTransition>>,
}

pub struct BuchiTransition {
    pub to: usize,
    pub conditions: Vec<(String, bool)>,  // (atomic_prop, must_be_true)
}
```

## Automaton Patterns

### Always (`[]p`)

```
States: {s0, s1}
Initial: s0
Accepting: {s0}
Transitions:
  s0 --p--> s0   (accepting loop)
  s0 --!p--> s1  (rejecting sink)
  s1 --any--> s1 (sink)
```

### Eventually (`<>p`)

```
States: {s0, s1}
Initial: s0
Accepting: {s1}
Transitions:
  s0 --p--> s1   (reach accepting)
  s0 --!p--> s0  (wait for p)
  s1 --any--> s1 (stay accepting)
```

### Next (`Xp`)

```
States: {s0, s1, s2}
Initial: s0
Accepting: {s1}
Transitions:
  s0 --any--> s1  (move to check state)
  s1 --p--> s1    (p holds, accepting)
  s1 --!p--> s2   (p doesn't hold, rejecting)
  s2 --any--> s2  (sink)
```

### Conjunction (`p && q`)

Product construction:

- States: cross-product of p-automaton and q-automaton
- Accepting: intersection of accepting sets
- Transitions: synchronized on shared atomic propositions

### Disjunction (`p || q`)

Product construction:

- States: cross-product of p-automaton and q-automaton
- Accepting: union of accepting sets
- Transitions: synchronized on shared atomic propositions

## Examples

### Simple Always

```rust
let formula = parse_ltl("[](x == 0)").unwrap();
let buchi = to_buchi(&formula).unwrap();
assert_eq!(buchi.num_states, 2);
assert_eq!(buchi.initial, 0);
assert_eq!(buchi.accepting, vec![0].into_iter().collect());
```

### Conjunction

```rust
let formula = parse_ltl("p && q").unwrap();
let buchi = to_buchi(&formula).unwrap();
assert_eq!(buchi.num_states, 4); // 2 × 2 product
// Accepting: both p and q are true
```

### Nested Temporal (Error)

```rust
let formula = parse_ltl("[]<>p").unwrap();
let result = to_buchi(&formula);
assert!(matches!(result, Err(LtlError::NestedTemporal { .. })));
```

## Testing

### Unit Tests

- Each pattern (`[]p`, `<>p`, `Xp`, `!p`, `p && q`, `p || q`)
- Product construction correctness
- Accepting state identification
- Transition coverage

### Integration Tests

- Construct automata for formulas from Spin models
- Verify automata accept/reject sample traces correctly
- Compare with Spin's automata for same formulas

### Property Tests

- Generated formulas → construct automaton → verify structure
- Automata size bounds (≤2^n states for n operators)

## Dependencies

- `ltl-parser` spec (for `LtlFormula` AST)

## Success Criteria

- ✅ Construct correct automata for all supported patterns
- ✅ Reject nested temporal formulas with clear errors
- ✅ Product construction produces correct accepting sets
- ✅ Performance meets R4 requirements
- ✅ Automata match Spin's for equivalent formulas
