# Tasks

## Phase 1: Dependency Metadata

- [x] 1.1 Emit `reads`, `writes` and `internal` on every generated transition
- [x] 1.2 Delete `src/por/stubborn.rs` and the label-parsing analysis in `src/por/mod.rs`
- [x] 1.3 Test: declared read/write sets match the effect closure for every corpus transition
- [x] 1.4 Make `--por` either drive a real implementation or fail loudly

## Phase 2: Reduction

- [ ] 2.1 Ample-set selection with C0, C1 and C2 over the metadata
- [ ] 2.2 C3 liveness proviso: no all-internal accepting cycle may be hidden
- [ ] 2.3 Test: reduced and unreduced searches agree on verdicts for every fixture
- [ ] 2.4 Per-claim budget: max_states bounds the run, and a starved claim reports StateBudget naming itself
- [ ] 2.5 Test: a dependent-guard chain is not wrongly reduced (an ample set must not skip a guard that reads another process's write)

## Phase 3: Differential Gating

- [ ] 3.1 Corpus fixtures: chain, concurrent-phase and independent-process shapes, plus the consumer's own chain shape with one and with two claims (one proctype per task, guarded on the predecessor's completion flag, body activate/deactivate/complete)
- [ ] 3.2 Harness asserting verdict equivalence and the parity metric against pan

## Phase 4: Parity

- [ ] 4.1 Enabled by default only after 3.2 passes on the whole corpus
- [ ] 4.2 Record the measured parity table in the benchmark corpus note
