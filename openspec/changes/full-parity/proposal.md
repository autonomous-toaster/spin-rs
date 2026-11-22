# Full Parity With pan

## Problem

The generated transition relation does not encode statement order. A proctype body of two
statements emits two transitions that are both enabled in the initial state, so the search
reaches states the program cannot reach and refutes models that are valid.

Baseline measured at `ca86943`, against pan, one verdict and error count per claim, over the
83 models pan can build:

| outcome | count | models |
|---|---|---|
| agree | 67 | step-machine shapes, channels, atomic, unless, liveness |
| **false refutation** | **3** | `order`, `x_beforeseq`, `x_twodo` |
| **false clean** | **1** | `adv_loop_and_assert` |
| decline (counted) | 8 | comparison atoms, `<>[]p`, chain shapes |
| refusal (hard error) | 1 | `nonflat` - multi-statement branch in a nested if |
| pan cannot build | 3 | pan's own C namespace collision |

The three structural cases are ordinary Promela and each one is a false refutation of a valid
model:

    bool a, b;
    active proctype P() { b = 1; a = 1 }
    ltl p { [] ( a -> b ) }                       ours 1 error, pan 0

    bool a, b;
    active proctype P() { b = 1; do :: (1) -> a = 1; break od }
    ltl p { [] ( a -> b ) }                       ours 1 error, pan 0

    bool a, b;
    active proctype P() { do :: (1) -> b = 1; break od;
                          do :: (1) -> a = 1; break od }
    ltl p { [] ( a -> b ) }                       ours 1 error, pan 0

The third is the phase-one-then-phase-two skeleton: the two phases run concurrently.

Root cause, read from the code rather than inferred. `emit_stmts` dispatches each statement to
an emitter that writes `guard = true`; nothing sequences them. The generator carries five separate
answers to "where is this process" - `_step_ <P>`, `_pc_ <P>`, `_atomic_step_ <P>`,
`_unless_step_ <P>`, and the flag `_done_ <P>` - and each construct invents its own convention
for using them. `_pc_` is documented as the program counter for `goto`/`break`/`label` and is read by
nothing on the body path. `emit_goto_stmt` sets `_done_ <P> = true` in order to jump, so the label
it targets is unreachable.

## What the generated corpus found

Phase 1 generated 65 models from the shape grammar and compared each against pan. The hand-written
corpus had 83 models and 4 wrong verdicts; the generated one has 65 and **30**:

| outcome | generated | hand-written |
|---|---|---|
| agree | 31 | 67 |
| false refutation | **25** | 3 |
| false clean | **5** | 1 |
| decline | 0 | 8 |
| refusal | 4 | 1 |

Three defect classes the hand corpus never reached, each hand-confirmed against pan:

- `atomic { b = 1; a = 1 }` is **not fused**: we reach `{a:0, b:1}` between the two assignments, where
  pan cannot. An atomic region is supposed to be one transition.
- A **terminating** model is reported as an accepting cycle: `b = 1; a = 1` with `<> ( a )` is
  refuted because a cycle is claimed to exist after the only process has finished. Checker-side.
- `for` loops **refuse** to run at all (all 4 refusals in the table).

So the hand corpus was not merely unrepresentative of structure; it was off by a factor of seven on
the number of wrong verdicts, and it had missed three defect classes entirely.

## What Phase 2 has done so far

The full CFG is not in. What landed is the part that removes the lies: a body of more than one
statement is routed through the step machine, which is the one place in the generator that
sequences correctly, with a trailing `break` so the last step terminates the process. Constructs
the step machine cannot flatten - a nested `do`, a `for`, a `goto`, an `atomic` region - are now
**refused** rather than reordered or silently advanced past.

| outcome | before Phase 1 | after Phase 2 so far |
|---|---|---|
| agree | 31 | 31 |
| false refutation | 25 | **4** |
| false clean | 5 | **2** |
| refusal | 4 | 28 |

Wrong verdicts: **30 to 6**. Refusals rose because a refusal is what an unsequenced construct now
does instead of being quietly reordered; those 28 are the CFG work (2.10-2.13), not regressions.

Three silent defects were found and fixed on the way, all of the same shape - a mechanism that is
declared and participates in nothing: the step machine dropped `atomic`/`unless`/`for` statements
with no effect, `goto` advanced past its target, `verify()` never called the `take_error()` that
`verify_with_config()` had always called, and a channel send inside a sequence emitted a vararg
call that could not load.

## The instrument is part of the problem

The previous measurement reported 74 of 75 models agreeing with pan. It was wrong in two ways.
It counted a model that we decline to decide as an agreement whenever our error count happened
to be 0, and it counted one such model as a mismatch as well. The corrected classifier in
`/tmp/srs/panall2.sh` separates five outcomes and puts declines in their own bucket, where they
belong: a decline is debt, and debt that is counted as agreement is invisible.

Neither corpus nor classifier could have found the ordering defect, because every model in the
corpus has exactly one statement in its proctype body. The measurement covered features; the
defect lives in structure.

## Parity metric

> For every model in the corpus and every claim, our verdict and error count SHALL equal pan's,
> or we SHALL decline with a counted `UnverifiedKind`. A decline SHALL never be counted as
> agreement, and a hard refusal SHALL never be counted as a verdict.

Raw state counts are not comparable: we evaluate every claim in one traversal, pan evaluates one
claim per traversal. State-count equality is explicitly not a goal.

## What changes

1. A structural model generator plus the pan oracle as the gate, built **first**, so every later
   change is measured rather than hoped for.
2. An execution model that compiles control flow to a CFG and emits one transition per edge, with
   one program counter per proctype.
3. Statement, data and property surface parity against pan, layer by layer.
4. An audit for mechanisms that are parsed, initialised or emitted and participate in nothing.

## Why the execution model comes first

Every other parity claim rests on "the reachable state set is right", which rests on "control
flow is right". While statements are unordered, a channel or `atomic` or liveness result that
agrees with pan can agree for the wrong reason: `test_verify_with_assertion` passed for months
only because the assert's guard accidentally supplied the ordering the model lacked. The CFG is
not the largest defect; it is the precondition for the other measurements meaning anything.

## Non-goals

- State-count equality with pan, in any form.
- New `UnverifiedKind` variants. `spin_rs::verify(&str) -> anyhow::Result<CheckResult>` stays
  source-compatible and the stable kind strings do not change.
- Performance work. The CFG may move state counts in either direction; only verdicts gate.
- pan's bitstate search, its state compression, and its memory management.
- Fairness (`fair`, `-f`) unless it is shown to change a verdict on some model. Marked human
  review only in `design.md`.

## Current state that must not be mistaken for a foundation

- `_pc_ <P>` is initialised, written by `goto` and `break`, read by `label` transitions and by the
  `atomic` expansion for its own internal purposes. It does not sequence a body.
- `do_exit_stack` is initialised and read but never pushed, so `break` always targets step 0.
- `ProctypeDef.provided` is parsed and referenced nowhere outside the AST; a `provided` clause is
  silently ignored.
- The step machine sequences a guard body correctly. That is the one place the design composes,
  and it is why every consumer-generated model agrees with pan.
