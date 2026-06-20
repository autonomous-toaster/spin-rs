## Context

spin-rs implements a Lua-based Promela model checker in Rust. Before it can replace the external Spin 6.5.x binary as veriplan's verification backend, we need empirical answers to two questions:

1. **Correctness equivalence**: Does spin-rs explore the same state space and detect the same violations as Spin?
2. **Performance**: Is spin-rs fast enough to replace Spin without unacceptable latency, including Spin's GCC compilation step?

The benchmark harness must be trustworthy (same models, same metrics), automated (single command), and reproducible (fixed seed, multiple runs).

## Goals / Non-Goals

**Goals:**

- Define a reusable model corpus of 15+ `.pml` models spanning three categories: plan-like (veriplan use case), classic Spin protocols, and edge cases
- Build a single `cargo run --bench --release` binary that runs all three phases and outputs both JSON and a human-readable table
- Phase 1: Compare state counts, error counts, and violation presence between spin-rs and Spin across exact+bitstate storage and DFS+BFS search
- Phase 2: Profile spin-rs's internal time breakdown — parse, codegen, Lua bootstrap, guard eval, effect exec, state serialize, hash lookup
- Phase 3: Wall-clock comparison (full pipeline + verification-only) reporting states/sec, transitions/sec, and speedup factor
- Verifiable — the harness SHALL abort with a clear error if outputs diverge beyond defined tolerance in Phase 1

**Non-Goals:**

- No channel-heavy models in Phase 1 (deferred)
- No modification of spin-rs's verification engine
- No CI integration or automated regression gate
- No comparison of Spin's `spin -a` parse step alone (always measured as part of full pipeline)

## Decisions

**D1: Single binary vs shell scripts.**
Chosen: `cargo run --bench --release`. A single Rust benchmark binary is more portable, reproducible, and maintainable than shell scripts. It invokes Spin via `std::process::Command` for comparison runs.

**D2: Model corpus as inline constants vs external .pml files.**
Chosen: External `.pml` files in `benches/models/`. This lets us reference the same files in both the Rust harness and manual `spin -a` invocations. Each model includes expected-state metadata in a companion JSON file or embedded comment.

**D3: Criterion vs hand-rolled timing.**
Chosen: Hand-rolled timing with `std::time::Instant`. Criterion is designed for microbenchmarks of stable functions; our benchmark spans multiple processes (Spin) and configurable models. Custom timing with warmup rounds gives us more control.

**D4: Tolerance criteria for equivalent outputs.**
Chosen: In exact+DFS mode, states_explored MUST match exactly (0% tolerance). In exact+BFS mode, states_explored MUST match exactly. In bitstate mode, a warning MAY fire but the test continues. If any model shows >1% deviation, the harness SHALL print a FAIL status and the model name. This catches real bugs while allowing for trivial scheduling differences.

**D5: How to measure Spin's "verify only" time.**
Chosen: Run `spin -a` + `gcc -O2 -o pan pan.c` once (the compilation step), then run `./pan -n` multiple times and take the median. The full pipeline time is the sum. This gives us both numbers without recompiling for every sample.

**D6: JSON output format.**
Chosen: Single JSON object per run with `{ phase, model, config, spin_rs: { states, transitions, errors, time_breakdown }, spin: { states, ... }, comparison: { states_match, errors_match, speedup, ... } }`. Human-readable table printed to stdout alongside.

## Risks / Trade-offs

**[Risk] Spin version skew:** Spin 6.5.2 may behave differently from newer versions. The harness SHALL log the Spin version at startup and pin to 6.5.x.
→ **Mitigation**: Document expected Spin version in harness output.

**[Risk] Model portability:** Not all Promela features are supported by spin-rs (channels deferred). Some models may parse in Spin but fail in spin-rs.
→ **Mitigation**: Tag each model with required feature set; skip models with unsupported features.

**[Risk] Compilation step variance:** GCC compilation time depends on system load, filesystem cache, and compiler version. This adds noise to full-pipeline measurements.
→ **Mitigation**: Report compilation time separately from verification time. Run multiple warmup rounds.

**[Risk] Non-deterministic state ordering:** spin-rs's Lua-based transitions and Spin's C transitions may enumerate interleavings in different orders, even with DFS. This could cause different state counts if POR or hash ordering differs.
→ **Mitigation**: For exact-match comparison, run both with POR disabled first. If states diverge, run BFS (more deterministic) to isolate the source of divergence.

**[Risk] Bitstate false positives:** spin-rs and Spin may have different false-positive rates due to different hash functions. State counts will never match.
→ **Mitigation**: Bitstate comparison only checks violation detection (errors match), not state count. A warning is emitted if state count differs >10%.
