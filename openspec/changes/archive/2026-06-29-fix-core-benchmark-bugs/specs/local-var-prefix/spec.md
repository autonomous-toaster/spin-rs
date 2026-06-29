## ADDED Requirements

### Task Reference

| Task ID | Description |
|---------|-------------|
| T1.1 | Fix `emit_guards` inline assignment effect to prefix targets with `current_proctype` |
| T1.2 | Fix `emit_assignment_effect` to prefix targets with `current_proctype` |
| T1.3 | Verify single_loop explores ~103 states after fix |

### Requirement: Guard body assignments prefix local variables

T1.1 SHALL ALWAYS prefix the assignment target with `current_proctype` when generating inline assignment effects inside `emit_guards`. When `current_proctype` is `Some("P")` and the target is `x`, the generated Lua SHALL be `s.P_x = value` instead of `s.x = value`.

#### Scenario: single_loop guard effect

- **WHEN** T1.1 generates code for `do :: x < 100 -> x = x + 1`
- **THEN** the effect SHALL write to `s.P_x` rather than `s.x`
- **AND** the guard SHALL read from `s.P_x` (already correct)

### Requirement: Atomic/d_step body assignments prefix local variables

T1.2 SHALL ALWAYS prefix the assignment target in `emit_assignment_effect` with `current_proctype`, matching the behavior of `emit_assignment`.

#### Scenario: Atomic block assignment

- **WHEN** T1.2 generates code for `atomic { x = x + 1 }` inside proctype `P`
- **THEN** the effect SHALL write to `s.P_x`

### Requirement: single_loop state count

T1.3 SHALL ALWAYS verify that `single_loop` model explores approximately 103 states after the prefix fix. The count MAY differ by up to 5% from Spin's expected 103.

#### Scenario: single_loop verification

- **WHEN** T1.3 runs `verify(SINGLE_LOOP)` after the prefix fix is applied
- **THEN** `result.states_explored` SHALL be within 5% of 103
- **AND** `result.errors` SHALL BE 0
