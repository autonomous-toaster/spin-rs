## ADDED Requirements

### Task Reference

| Task ID | Description |
|---------|-------------|
| T4.1 | Expand for loops at parse time into sequential statements |
| T4.2 | Verify token_ring_n5 init block correctly initializes channels |

### Requirement: For loop parsed as sequential expansion

T4.1 SHALL ALWAYS parse `for (<var> in <start> .. <end>) { <body> }` and expand at parse time into `<var> = <start>; <body>; <var> = <start>+1; <body>; ...; <var> = <end>; <body>`. The loop variable SHALL be a regular variable declaration (not scoped to the loop body).

#### Scenario: for (i in 0 .. 4)

- **WHEN** T4.1 parses `for (i in 0 .. 4) { tok[i] = [1] of { byte } }`
- **THEN** the AST SHALL contain 5 sequential statements:
          `i = 0; tok[0] = [1] of { byte };`
          `i = 1; tok[1] = [1] of { byte };`
          `...`
          `i = 4; tok[4] = [1] of { byte };`

### Requirement: token_ring_n5 init works

T4.2 SHALL ALWAYS verify that `token_ring_n5` model parses and initializes correctly after the for loop fix, matching the current benchmark expectation of 0 errors.

#### Scenario: token_ring_n5 verification

- **WHEN** T4.2 runs `verify(TOKEN_RING_N5)` after for loop fix
- **THEN** the model SHALL parse without error
- **AND** verification SHALL complete without errors
