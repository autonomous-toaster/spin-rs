## ADDED Requirements

### Task Reference

| Task ID | Description |
|---------|-------------|
| T6.1 | Collect inline definitions into a HashMap during codegen |
| T6.2 | Expand inline calls at codegen time via parameter substitution |
| T6.3 | Verify dining_n4 model parses and explores non-trivial state space |

### Requirement: Inline definitions collected

T6.1 SHALL ALWAYS collect all `TopLevel::Inline` definitions from the AST into a HashMap keyed by inline name during codegen initialization. Each definition SHALL store the parameter names and body statements.

#### Scenario: Inline definitions parsed

- **WHEN** T6.1 processes `inline pickup(i) { atomic { (fork[i] == 0); fork[i] = 1 } }`
- **THEN** an entry SHALL be stored with key `"pickup"`, parameters `["i"]`, and body containing the atomic block

### Requirement: Inline calls expanded at call site

T6.2 SHALL ALWAYS expand inline calls when encountering an inline name used as a statement or in a guard expression. Expansion SHALL substitute each parameter name with the corresponding argument expression. The substituted body SHALL be emitted in place of the call.

#### Scenario: pickup(_pid) expanded

- **WHEN** T6.2 encounters `pickup(_pid)` and the inline definition is `pickup(i) { atomic { (fork[i] == 0); fork[i] = 1 } }`
- **THEN** the generated code SHALL be equivalent to
          `atomic { (fork[_pid] == 0); fork[_pid] = 1 }`

### Requirement: dining_n4 explores state space

T6.3 SHALL ALWAYS verify that `dining_n4` model explores more than 1 state after the inline expansion fix.

#### Scenario: dining_n4 verification

- **WHEN** T6.3 runs `verify(DINING_N4)` after inline expansion
- **THEN** `result.states_explored` SHALL BE greater than 1
- **AND** `result.errors` SHALL equal 0 (no deadlock in unmodified dining philosophers)
