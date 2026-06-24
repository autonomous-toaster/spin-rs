## Task Reference

| Task ID | Description |
|---------|-------------|
| T3.1 | Distinguish guard context vs assignment context in codegen |
| T3.2 | Apply `~= 0` check ONLY in guard/condition expressions |
| T3.3 | Remove `~= 0` from assignment expressions |
| T3.4 | Test: `single_loop` returns to 102+ states |

## ADDED Requirements

### Requirement: Context tracking in codegen

T3.1 SHALL add context tracking to distinguish guard expressions from assignment expressions. The codegen SHALL track whether it's generating code for:

- **Guard context**: Boolean conditions in `if`, `do`, `unless` guards
- **Assignment context**: Right-hand side of assignments, function arguments, etc.

#### Scenario: Guard context identified

- **WHEN** T3.1 generates code for `do :: (x && y) -> ...`
- **THEN** it SHALL mark the expression as being in guard context

#### Scenario: Assignment context identified

- **WHEN** T3.1 generates code for `x = y + 1`
- **THEN** it SHALL mark the expression as being in assignment context

### Requirement: Boolean check in guards

T3.2 SHALL apply `~= 0` check to boolean variables ONLY in guard context. This is required because:

- In Promela/C: `0` is falsy, non-zero is truthy
- In Lua: `0` is truthy!
- Guard: `if s.x and s.y` evaluates TRUE when both are 0 (wrong!)
- Correct: `if s.x ~= 0 and s.y ~= 0`

For each identifier in guard context:

- If the variable is boolean type, emit `s.var_name ~= 0`
- If the variable is integer type, emit `s.var_name` (no check needed)

#### Scenario: Boolean guard check

- **WHEN** T3.2 generates guard for `do :: (flag) -> ...` where `flag` is bool
- **THEN** it SHALL emit `guard = function(s) return s.flag ~= 0 end`

#### Scenario: Integer guard (no check)

- **WHEN** T3.2 generates guard for `do :: (x > 0) -> ...` where `x` is int
- **THEN** it SHALL emit `guard = function(s) return s.x > 0 end` (no ~= 0 on x)

### Requirement: No boolean check in assignments

T3.3 SHALL NOT apply `~= 0` check in assignment context. Assignments SHALL emit plain variable references.

#### Scenario: Assignment expression

- **WHEN** T3.3 generates `x = y + 1`
- **THEN** it SHALL emit `s.x = s.y + 1` (NOT `s.y ~= 0 + 1`)

#### Scenario: Function argument

- **WHEN** T3.3 generates `printf("%d", x)`
- **THEN** it SHALL emit `printf("%d", s.x)` (NOT `s.x ~= 0`)

### Requirement: single_loop regression test

T3.4 SHALL verify that `single_loop` model returns to 102+ states after the boolean scoping fix. The model has:

- A single process with a loop: `do :: x < 100 -> x = x + 1 :: x >= 100 -> break od`
- Expected: 102-103 states (Spin reports 103)

#### Scenario: single_loop state count

- **WHEN** T3.4 runs `verify(SINGLE_LOOP)`
- **THEN** the result SHALL have `states_explored >= 102`
- **AND** the result SHALL have `errors == 0`
