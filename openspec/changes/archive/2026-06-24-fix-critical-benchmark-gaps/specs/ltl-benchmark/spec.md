## Task Reference

| Task ID | Description |
|---------|-------------|
| T2.1 | Detect LTL formulas in parsed models |
| T2.2 | Wire `verify_ltl()` into benchmark comparison |
| T2.3 | Compare spin-rs LTL results against Spin's error counts |
| T2.4 | Test: `ltl_violation` detects exactly 1 error |

## ADDED Requirements

### Requirement: LTL detection in benchmark

T2.1 SHALL detect LTL formulas in the parsed model. The benchmark SHALL iterate through `model.declarations` and identify `TopLevel::Ltl` entries.

#### Scenario: LTL formula detected

- **WHEN** T2.1 processes a model with `ltl p0 { [](x == 0) }`
- **THEN** it SHALL identify the LTL formula for verification

### Requirement: LTL verification integration

T2.2 SHALL call `verify_ltl(source, formula, property_name)` for each LTL formula in the model. The benchmark SHALL:

- Extract the LTL formula string from `LtlFormula.formula`
- Use the LTL name (or "unnamed" if no name) as `property_name`
- Collect violations from `verify_ltl()` results

#### Scenario: LTL verification called

- **WHEN** T2.2 benchmarks a model with LTL formulas
- **THEN** it SHALL call `verify_ltl()` for each formula
- **AND** collect the results for comparison

### Requirement: Error count comparison

T2.3 SHALL compare spin-rs LTL results against Spin's error counts. For models with LTL violations:

- Spin reports `errors: 1` for each violated property
- spin-rs SHALL report the same error count
- Tolerance: 0% (exact match required for LTL violations)

#### Scenario: LTL violation detected equivalently

- **WHEN** Spin finds 1 LTL violation on `ltl_violation` model
- **THEN** spin-rs SHALL also find 1 error
- **AND** T2.3 SHALL report PASS

#### Scenario: LTL false negative

- **WHEN** spin-rs reports 0 errors but Spin reports >0 errors
- **THEN** T2.3 SHALL report FAIL with model name and expected error count

### Requirement: ltl_violation test

T2.4 SHALL verify that `ltl_violation` model produces exactly 1 error. The model has:

- A process that toggles `x` between 0 and 1
- LTL property `[](x == 0)` (always x equals 0)
- This property is violated when x becomes 1

#### Scenario: LTL violation detected

- **WHEN** T2.4 runs `verify_ltl(LTL_VIOLATION_SOURCE, "[](x == 0)", "p0")`
- **THEN** the result SHALL have `errors == 1`
- **AND** the violation SHALL describe the LTL property violation
