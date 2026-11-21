# Partial Order Reduction

## Problem

The engine has no partial order reduction. On every model with concurrent or independent
processes it explores the full interleaving product, where pan explores a reduced set.

Measured at v0.3.4. The models are: a chain of N proctypes each guarded on its
predecessor's completion flag with body / activate / deactivate / complete / break; and N
proctypes with guard `(1)` and three assignments each, which is the shape of a `[concurrent]`
phase. T3.1 commits both as fixtures.

| fixture | ours | pan (POR on) | pan (`-DNOREDUCE`) |
|---|---|---|---|
| chain, 4/6/8 tasks | 17 / 25 / 33 | 21 / 31 / 41 | 21 / 31 / 41 |
| 5 independent procs, 3 stmts each | 3125 | 1370 | 3906 |
| 5 independent procs, claim on 1 | 3125 | 23 | 3906 |

Two facts follow, and both matter for scoping:

1. With POR disabled pan is **worse** than us (3906 vs 3125) on independent processes, and
   worse on chains (41 vs 33). Our folding of the trailing `break` and single-pass
   evaluation of all claims are already wins.
2. Therefore pan's entire advantage is POR. Nothing else needs to change to reach parity.

## Parity metric

Raw `states stored` is not comparable between the two engines, because we evaluate every
claim in one traversal and pan evaluates one claim per traversal (`-N pN`). Comparing them
directly produced a false 65536-vs-31 result during this investigation.

Parity is therefore defined as:

> For every fixture in the corpus and every property, our error counts match pan's per-claim
> error counts; and our states explored for a complete all-property verdict is no greater
> than the sum of pan's `states stored` across the per-claim traversals needed for the same
> verdict.

The second clause is the one POR must satisfy. It is deliberately expressed as a sum, because
that is the work a caller of `pan` would actually pay.

## What changes

1. Replace the dormant `src/por/` and the no-op `--por` flag with a real implementation.
2. Emit per-transition dependency metadata from the code generator, replacing the current
   label-string heuristic.
3. Ample-set selection satisfying C0-C3 with the liveness cycle proviso.
4. Behind a flag, default off, gated by an equivalence check against both pan and the
   unreduced search.

## Non-goals

- No change to the state representation, the step machine, or the folding of trailing
  `break`. Those are already better than pan.
- No statement merging. Measured neutral (`-o3` does not change pan's counts).
- No write-only variable hiding. Measured neutral (`-o2` changes vector size, not counts).
- No change to which properties are supported.
- No fairness work.

## Current state that must not be mistaken for a foundation

`src/por/mod.rs` (507 lines) plus `src/por/stubborn.rs` (166 lines) look like an
implementation and are not one:

- `analyze_transition` infers read/write sets by string-parsing the transition label, expecting
  forms such as `"P:x=1"`. Generated labels are `"do:sel0"` and `"do:step1"`, so every
  transition is recorded as writing the variable `do`. Every pair then conflicts and the ample
  set degenerates to "all transitions".
- `compute_ample_set_with_c3` returns every transition whenever any transition is visible, and
  in these models nearly every step writes a global.
- `local` is inferred from `!label.contains('!')`, which has nothing to do with whether the step
  touches globals.

`--por` sets `CheckerConfig::por_enabled`, which no code path reads.

Wiring this up as-is would either do nothing or, if the label heuristic ever matched, skip
violations silently. It is to be deleted, not extended.
