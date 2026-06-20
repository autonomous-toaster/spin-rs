## 1. Correctness Equivalence (Phase 1)

- [x] 1.1 Create model corpus: at least 5 `.pml` files with JSON companion files (expected states, errors, violations) covering plan-like safety, multi-process, deadlock, LTL-violation, and assertion models
- [x] 1.2 Implement Spin runner: wrap `spin -a + gcc + ./pan -n`, parse text output into structured result (states, transitions, errors, violations)
- [x] 1.3 Implement spin-rs runner: call `verify()` or `CheckerBuilder` with matching config, return same structured result
- [x] 1.4 Implement comparator: compare state counts (exact ±0% for no-POR, ≤1% for POR), error counts, and violation lists; output PASS/FAIL/WARN per model
- [x] 1.5 Add bitstate comparison mode: compare error detection only (state count mismatch is INFO, not FAIL)
- [x] 1.6 Add BFS comparison mode: same tolerance rules as DFS, verify BFS exact match with `-DS_BFS`

## 2. Local Performance (Phase 2)

- [x] 2.1 Add `#[cfg(feature = "bench")]` timing probes to spin-rs pipeline (parse, codegen, Lua bootstrap, verify), readable via a `BenchTiming` struct
- [x] 2.2 Implement parse throughput benchmark (chars/sec over model corpus)
- [x] 2.3 Implement codegen throughput benchmark (AST nodes/sec)
- [x] 2.4 Implement Lua bootstrap time measurement (ms to init mlua + prelude)
- [x] 2.5 Implement guard evaluation throughput measurement (guards/sec across full state space)
- [x] 2.6 Implement effect execution throughput measurement (effects/sec)
- [x] 2.7 Implement state serialization cost measurement (bytes/state, states serialized/sec)
- [x] 2.8 Implement hash+lookup throughput measurement (fxhash + ExactStore inserts/sec)
- [x] 2.9 Implement Lua↔Rust FFI roundtrip benchmark (nanoseconds per empty call)
- [x] 2.10 Aggregate breakdown table with bottleneck indicators, scale-reporting (constant/linear/super-linear across model sizes)

## 3. Global Comparison (Phase 3)

- [x] 3.1 Create Spin pipeline wrapper: `spin -a + gcc -O2 + ./pan -n` with structured output parsing, Spin version detection at startup
- [x] 3.2 Create spin-rs pipeline wrapper: `spin_rs::verify()` with equivalent config flags
- [x] 3.3 Implement full pipeline wall-clock measurement: warmup + 5 runs + median, report both times and speedup
- [x] 3.4 Implement verification-only measurement: compile once, run 5 times, take median; report compilation overhead separately
- [x] 3.5 Compute and report states/sec, transitions/sec, memory/state, speedup factor
- [x] 3.6 Run cross-configuration sweep: all models × (Exact/Bitstate) × (DFS/BFS) × (POR on/off) × (LTL present/absent)
- [x] 3.7 Output human-readable table to stdout + JSON to `target/bench-results/<timestamp>.json`
