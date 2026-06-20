## Task Reference

| Task ID | Description |
|---------|-------------|
| T3.1 | Parse multi-variable declarations: `bool a, b, c;` |
| T3.2 | Parse `active [N] proctype name()` — array process instantiation |
| T3.3 | Add `_pid` built-in variable support in codegen |
| T3.4 | Parse `init { }` block |
| T3.5 | Parse `inline` definitions (store in model, skip expansion) |
| T3.6 | Parse `for (i in 0 .. N) { }` loops |
| T3.7 | Parse `else` guard keyword in if/do blocks |

## ADDED Requirements

### Requirement: Multi-variable declarations

T3.1 SHALL ALWAYS parse `bool a, b, c;` into separate `VarDecl` nodes. T3.1 SHALL use `separated_list1(symbol(","), ident)` after the type to collect multiple names.

#### Scenario: Three bools

- **WHEN** T3.1 parses `bool a, b, c;`
- **THEN** three `TopLevel::GlobalVar` nodes SHALL BE created

#### Scenario: Mixed with init

- **WHEN** T3.1 parses `byte x = 5, y;`
- **THEN** `x` SHALL have init `5` and `y` SHALL have no init

### Requirement: Array process instantiation

T3.2 SHALL ALWAYS parse `active [N] proctype name() { ... }` and expand to N `ProctypeDef` nodes, each with a unique name (e.g., `name_0`, `name_1`, ..., `name_{N-1}`). Each instance SHALL have `_pid = i`.

#### Scenario: Two instances

- **WHEN** T3.2 parses `active [2] proctype user() { byte x; }`
- **THEN** two proctypes SHALL BE created: `user_0` and `user_1`

### Requirement: _pid variable

T3.3 SHALL ALWAYS inject `_pid` as a pre-initialized variable in each proctype instance's state. `_pid` SHALL be set to the instance index (0-indexed within the array instantiation, or 0 for singleton proctypes).

#### Scenario: _pid referenced in guard

- **WHEN** T3.3 generates code for `active [2] proctype user() { (flag[1-_pid] == 0); }`
- **THEN** `state._pid` SHALL be set to `0` for instance 0 and `1` for instance 1
- **AND** the guard `s.flag[1 - s._pid] == 0` SHALL reference the correct array element

### Requirement: init block

T3.4 SHALL ALWAYS parse `init { <stmts> }` into `TopLevel::Init(InitDef { body })`. The init block SHALL generate transitions similarly to a proctype, but SHALL run once on startup.

### Requirement: inline definitions

T3.5 SHALL ALWAYS parse `inline name(params) { body }` into a new `TopLevel::InlineDef` variant. Inline expansion (macro substitution) is deferred; the model stores the definition for reference.

### Requirement: for loops

T3.6 SHALL ALWAYS parse `for (<var> in <start> .. <end>) { <body> }` and expand into sequential statements at parse time. The variable SHALL iterate from `start` to `end` inclusive.

### Requirement: else guard

T3.7 SHALL ALWAYS parse `:: else -> <body>` as a guard with no condition. In the codegen, `else` guards SHALL evaluate to `true` only when no other guard in the same if/do block is enabled.
