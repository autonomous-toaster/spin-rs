## Task Reference

| Task ID | Description |
|---------|-------------|
| T1.1 | Create model corpus of `.pml` files for correctness testing |
| T1.2 | Implement Spin runner for correctness comparison |
| T1.3 | Implement spin-rs runner with same configuration |
| T1.4 | Compare state counts, error counts, and violation lists |
| T1.5 | Define and implement tolerance criteria |
| T1.6 | Handle bitstate mode comparison (error-only, not state count) |

## ADDED Requirements

### Requirement: Model corpus for correctness testing

T1.1 SHALL complete BEFORE T1.2 SHALL run Spin on the corpus. T1.1 SHALL define at least 5 `.pml` models covering:

- A plan-like model with 5 boolean state variables and 3 LTL properties (veriplan use case)
- A simple safety model (single proctype with assertion)
- A multi-process model (2 proctypes, synchronized via shared variable)
- A model with a known deadlock
- A model with a known LTL violation

T1.1 SHALL ALWAYS include expected values (states, errors, violations) as a JSON companion file per model.

#### Scenario: Model runs in spin-rs

- **WHEN** T1.3 runs each model from T1.1 with exact storage and DFS
- **THEN** T1.3 SHALL return state count, error count, and violation list

#### Scenario: Model runs in Spin

- **WHEN** T1.2 runs `spin -a model.pml` followed by `gcc -O2 -o pan pan.c` and `./pan -n`
- **THEN** T1.2 SHALL parse the Spin output to extract state count, error count, and violation list

### Requirement: State count matching with exact storage

T1.4 SHALL compare spin-rs and Spin state counts AFTER T1.3 and T1.2 SHALL complete. T1.4 SHALL ALWAYS match state counts exactly (0% tolerance) when POR is disabled and storage is exact. T1.4 SHALL allow up to 1% deviation when POR is enabled (ample sets MAY differ between implementations).

#### Scenario: Exact match, POR off

- **WHEN** T1.3 runs a model with StorageMode::Exact, SearchMode::DepthFirst, POR disabled
- **AND** T1.2 runs the same model with `./pan -n -DNOREDUCE`
- **THEN** T1.4 SHALL report PASS if states_explored differs by 0%

#### Scenario: Exact match, POR on

- **WHEN** T1.3 runs a model with the same configuration but POR enabled
- **AND** T1.2 runs the same model with `./pan -n` (POR enabled by default)
- **THEN** T1.4 SHALL report PASS if states_explored differs by ≤1%
- **AND** T1.4 SHALL report WARN if states_explored differs by >1% (possible POR algorithm divergence)

### Requirement: Error detection equivalence

T1.4 SHALL ALWAYS compare error counts and violation descriptions. For models with expected violations, spin-rs SHALL detect the same violations as Spin (same number, same property names). False negatives (spin-rs misses a violation) SHALL result in FAIL. False positives SHALL result in FAIL.

#### Scenario: Deadlock detected equivalently

- **WHEN** T1.2 finds 1 deadlock error on the deadlock model
- **THEN** T1.3 SHALL also find 1 error
- **AND** T1.4 SHALL report PASS

#### Scenario: LTL violation detected equivalently

- **WHEN** T1.2 finds the LTL property violated on the LTL-violation model
- **THEN** T1.3 SHALL also find the same LTL property violated

#### Scenario: Missing violation

- **WHEN** T1.3 reports 0 errors but T1.2 reports >0 errors
- **THEN** T1.4 SHALL report FAIL with the model name and expected error count

### Requirement: Bitstate mode comparison

T1.6 SHALL compare Spin and spin-rs in bitstate mode AFTER T1.2 and T1.3 SHALL run. T1.6 SHALL ALWAYS compare error detection only — state counts are NOT expected to match due to different hash functions. T1.6 SHALL report PASS when both find the same errors, with a note on state count difference.

#### Scenario: Bitstate error detection

- **WHEN** T1.3 runs a model with StorageMode::Bitstate
- **AND** T1.2 runs the same model with `./pan -n -DBSIZE=1024`
- **THEN** T1.6 SHALL report PASS if both detect the same set of errors
- **AND** T1.6 SHALL emit state count difference as INFO (not WARN or FAIL)

### Requirement: BFS mode comparison

T1.4 SHALL compare spin-rs BFS and Spin BFS state counts AFTER T1.3 and T1.2 SHALL run with BFS. BFS is inherently more deterministic than DFS; state counts SHALL match exactly (±0) when POR is disabled.

#### Scenario: BFS exact match

- **WHEN** T1.3 runs a model with SearchMode::BreadthFirst and POR off
- **AND** T1.2 runs the same model with `./pan -n -DS_BFS`
- **THEN** T1.4 SHALL report PASS if states_explored matches exactly
