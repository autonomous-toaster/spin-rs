## 1. LTL Parser Implementation

- [x] 1.1 Create `ltl2ba-rs-simplified` module structure in `src/property/ltl2ba/`
- [x] 1.2 Implement `LtlFormula` enum (True, False, Atom, Not, And, Or, Always, Eventually, Next)
- [x] 1.3 Implement recursive descent parser for LTL strings
- [x] 1.4 Support temporal operators: `[]`, `<>`, `X`, `G`, `F`, `O`
- [x] 1.5 Support boolean operators: `&&`, `||`, `!`, `->`
- [x] 1.6 Support atomic propositions (variable comparisons)
- [x] 1.7 Implement `LtlError` enum with `UnsupportedOperator`, `NestedTemporal`, `ParseError`
- [x] 1.8 Add clear error messages with position information
- [x] 1.9 Write unit tests for parser (valid and invalid formulas)

## 2. Büchi Construction Implementation

- [x] 2.1 Implement `BuchiAutomaton` struct (num_states, initial, accepting, transitions)
- [x] 2.2 Implement `BuchiTransition` struct (to, conditions)
- [x] 2.3 Implement pattern matcher for `[]p` (always)
- [x] 2.4 Implement pattern matcher for `<>p` (eventually)
- [x] 2.5 Implement pattern matcher for `Xp` (next)
- [x] 2.6 Implement pattern matcher for `!p` (negation)
- [x] 2.7 Implement product construction for `p && q` (conjunction)
- [x] 2.8 Implement product construction for `p || q` (disjunction)
- [x] 2.9 Implement nested temporal detection (reject `[]<>p`, etc.)
- [x] 2.10 Write unit tests for each pattern (verify automaton structure)

## 3. Product Construction Implementation

- [x] 3.1 Implement `ProductState<S>` struct with cached hash
- [x] 3.2 Implement `ProductTransition<S>` struct
- [x] 3.3 Implement atomic proposition evaluation from model state
- [x] 3.4 Implement transition synchronization (model × Büchi)
- [x] 3.5 Handle multiple enabled Büchi transitions per model transition
- [x] 3.6 Write unit tests for product construction
- [x] 3.7 Write integration tests (simple model × simple Büchi)

## 4. Nested DFS Implementation

- [x] 4.1 Implement `NestedDFS<S>` struct (visited1, visited2, stack, trail)
- [x] 4.2 Implement outer DFS (dfs1) for exploring product space
- [x] 4.3 Implement inner DFS (dfs2) for cycle detection
- [x] 4.4 Implement violation construction with error trail
- [x] 4.5 Support max depth limit for termination
- [x] 4.6 Write unit tests for nested DFS
- [x] 4.7 Write integration tests (liveness violations and holds)

## 5. Integration with spin-rs

- [x] 5.1 Update `BuchiAutomaton::from_ltl()` to use simplified implementation
- [x] 5.2 Update `verify_ltl()` function to use product construction + nested DFS
- [x] 5.3 Update `PropertyChecker` to use new LTL pipeline
- [x] 5.4 Update CLI `--ltl` flag to work with new implementation
- [x] 5.5 Update documentation (README, module docs) with limitations
- [x] 5.6 Add examples for supported LTL formulas
- [x] 5.7 Write integration tests against Promela models

## 6. Testing and Validation

- [x] 6.1 Test parser against all supported operators
- [x] 6.2 Test Büchi construction against known automata
- [x] 6.3 Test product construction correctness
- [x] 6.4 Test nested DFS on liveness examples
- [x] 6.5 Compare results with Spin on standard models (Peterson, leader election)
- [x] 6.6 Performance benchmarks (parse time, construction time, search time)
- [x] 6.7 Document known limitations and unsupported operators

## 7. Documentation

- [x] 7.1 Write crate-level documentation for `ltl2ba-rs-simplified`
- [x] 7.2 Document supported operators with examples
- [x] 7.3 Document unsupported operators with workarounds
- [x] 7.4 Add API documentation (rustdoc)
- [x] 7.5 Update spin-rs README with LTL capabilities
- [x] 7.6 Write migration guide (simplified → full ltl2ba in future)
