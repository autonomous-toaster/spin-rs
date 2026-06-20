## Task Reference

| Task ID | Description |
|---------|-------------|
| T3.1 | Create Spin wrappers for pipeline execution (spin -a, gcc, ./pan) |
| T3.2 | Create spin-rs wrappers for pipeline execution |
| T3.3 | Measure full pipeline wall-clock time (both tools) |
| T3.4 | Measure verification-only wall-clock time (both tools) |
| T3.5 | Report states/sec, transitions/sec, speedup factor |
| T3.6 | Run comparison across all model sizes and configurations |
| T3.7 | Output JSON + human-readable table |

## ADDED Requirements

### Requirement: Spin pipeline wrapper

T3.1 SHALL complete BEFORE T3.3 SHALL measure full pipeline time. T3.1 SHALL run:

1. `spin -a model.pml` (parse + C code generation)
2. `gcc -O2 -o pan pan.c` (compilation to native binary)
3. `./pan -n` (verification step)

T3.1 SHALL capture stdout and stderr of each step. T3.1 SHALL fail hard if any step returns non-zero exit code. T3.1 SHALL report the Spin version string at harness startup.

#### Scenario: Spin pipeline succeeds

- **WHEN** T3.1 runs a valid model.pml through the Spin pipeline
- **THEN** T3.1 SHALL return parsed output (states, transitions, errors, violations)

#### Scenario: Spin pipeline fails on invalid model

- **WHEN** T3.1 runs a malformed model.pml
- **THEN** T3.1 SHALL return an error with the stderr output from the failing step

### Requirement: spin-rs pipeline wrapper

T3.2 SHALL complete BEFORE T3.3 SHALL measure full pipeline time. T3.2 SHALL run `spin_rs::verify()` on the model and return structured output. T3.2 SHALL support the same configuration flags as T3.1 (search mode, storage mode, POR on/off).

#### Scenario: spin-rs pipeline succeeds

- **WHEN** T3.2 verifies a valid model with `spin_rs::verify()`
- **THEN** T3.2 SHALL return parsed output (states, transitions, errors, violations)

### Requirement: Full pipeline wall-clock comparison

T3.3 SHALL measure and compare end-to-end wall-clock time AFTER T3.1 and T3.2 SHALL complete. T3.3 SHALL report both times with the same measurement methodology:

- Warmup: discard first run
- Measurement: run 5 times, take median
- Unit: seconds (with millisecond precision)

#### Scenario: Full pipeline on plan-like model (5 tasks, 3 LTLs)

- **WHEN** T3.3 runs the small plan model
- **THEN** T3.3 SHALL report spin-rs end-to-end time AND Spin end-to-end time (spin -a + gcc + ./pan)
- **AND** T3.3 SHALL compute speedup = Spin_time / spin_rs_time

#### Scenario: Full pipeline on large model (50 tasks, 20 LTLs)

- **WHEN** T3.3 runs the large plan model
- **THEN** T3.3 SHALL report both times and speedup factor

#### Scenario: Full pipeline on protocol model

- **WHEN** T3.3 runs Petersen N=3
- **THEN** T3.3 SHALL report both times and speedup factor

### Requirement: Verification-only comparison

T3.4 SHALL measure verification time only (excluding compilation) AFTER T3.1 and T3.2 SHALL instrument verification steps. For Spin, T3.4 SHALL re-use the compiled `pan` binary from T3.1 (compile once, run many). For spin-rs, T3.4 SHALL re-use the Lua model (compile once, run many).

T3.4 SHALL run `./pan -n` (Spin) or `checker.check()` (spin-rs) 5 times and take the median. T3.4 SHALL run AFTER T3.3 SHALL have compiled the pan binary.

#### Scenario: Verification-only comparison

- **WHEN** T3.4 runs on a model where `pan` is already compiled
- **THEN** T3.4 SHALL report the median verification-only time for both tools
- **AND** T3.4 SHALL report the compilation time separately as "compilation overhead"

### Requirement: States/sec and transitions/sec reporting

T3.5 SHALL compute throughput metrics AFTER T3.4 SHALL measure verification time. T3.5 SHALL report:

- `states/sec`: total states explored / verification time
- `transitions/sec`: total transitions / verification time
- `memory/state`: estimated peak memory / states_stored (for exact mode)
- `speedup`: ratio of Spin verification-only time to spin-rs verification-only time

T3.5 SHALL complete BEFORE T3.6 SHALL run cross-configuration comparisons.

#### Scenario: Throughput report

- **WHEN** T3.5 computes metrics for all models
- **THEN** T3.5 SHALL output a table with model, tool, states/sec, trans/sec, and speedup

### Requirement: Cross-configuration comparison

T3.6 SHALL run comparison across all model sizes and all configurations AFTER T3.5 SHALL report basic metrics. T3.6 SHALL sweep:

- All models from the corpus (small, medium, large plan; protocol; edge cases)
- Storage modes: Exact, Bitstate
- Search modes: DFS, BFS
- POR: on, off
- LTL: present, absent

T3.6 SHALL complete BEFORE T3.7 SHALL format the output.

#### Scenario: Full sweep

- **WHEN** T3.6 runs the full configuration matrix on each model
- **THEN** T3.6 SHALL print a progress indicator for each (model, config) pair
- **AND** T3.6 SHALL aggregate results for JSON output

### Requirement: JSON and human-readable output

T3.7 SHALL format benchmark results AFTER T3.6 SHALL complete comparison. T3.7 SHALL produce two outputs:

1. **stdout**: Human-readable table showing each (model, config) pair with states, errors, and throughput
2. **JSON file**: Full structured output with all measurements for machine consumption

T3.7 SHALL print to stdout and write JSON to `target/bench-results/<timestamp>.json`.

#### Scenario: Human-readable output

- **WHEN** T3.7 formats results
- **THEN** stdout SHALL show a table with columns: Model, Config, Tool, States, Errors, Time (s), States/sec, Speedup

#### Scenario: JSON output

- **WHEN** T3.7 writes JSON
- **THEN** the JSON SHALL include: timestamp, spin-version, host info, and an array of result objects with all raw measurements
