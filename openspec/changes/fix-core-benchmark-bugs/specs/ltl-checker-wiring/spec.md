## ADDED Requirements

### Task Reference

| Task ID | Description |
|---------|-------------|
| T5.1 | Wire `PropertyChecker::check_liveness` into `Checker::check_dfs` |
| T5.2 | Report LTL violations in `CheckResult.violations` |
| T5.3 | Verify ltl_violation model reports 1 error after wiring |

### Requirement: LTL verification runs during check

T5.1 SHALL ALWAYS call `PropertyChecker::check_liveness` after the DFS/BFS exploration phase when the model has LTL formulas. The property checker SHALL use the same model instance used for exploration (not a fresh parse).

#### Scenario: check_dfs with LTL formula

- **WHEN** T5.1 runs `check_dfs` on a model that has LTL formulas
- **THEN** the function SHALL invoke `PropertyChecker::check_liveness` for each formula
- **AND** SHALL NOT short-circuit or skip this step

### Requirement: LTL violations reported

T5.2 SHALL ALWAYS add LTL violations to `CheckResult.violations`. Each LTL violation SHALL have `property_name` set to the LTL formula name (e.g., "p0") and `description` containing the violation details.

#### Scenario: LTL violation found

- **WHEN** T5.2 detects a violation for LTL formula `p0: [](x == 0)`
- **THEN** `result.violations` SHALL contain at least one entry with `property_name == "p0"`
- **AND** `result.errors` SHALL be incremented by 1

### Requirement: ltl_violation errors match expected

T5.3 SHALL ALWAYS verify that `ltl_violation` model reports exactly 1 error after the LTL wiring fix.

#### Scenario: ltl_violation verification

- **WHEN** T5.3 runs `verify(LTL_VIOLATION)` after LTL wiring
- **THEN** `result.errors` SHALL equal 1
