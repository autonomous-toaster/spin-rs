# Design

## Temporal categories

| Requirement | Category | Tasks |
|---|---|---|
| Dependency Metadata | BEFORE | T1.1 before T1.3 |
| Reduction Soundness | BEFORE | T2.1 before T2.3 |
| Differential Gating | BEFORE | T3.1 before T3.2 |
| Parity | BEFORE | T3.2 before T4.1 |

## Why POR and not something else

Measured: with `-DNOREDUCE`, pan stores 3906 states on a 5-process model where we store 3125,
and 41 states on an 8-task chain where we store 33. All of pan's remaining advantage appears
only when reduction is enabled. Statement merging (`-o3`) and write-only hiding (`-o2`) were
both measured neutral. So there is exactly one mechanism to build.

## Dependency metadata (T1.1)

The code generator knows, at emission time, exactly which variables a guard reads and which an
effect writes. Parse nothing back out of strings.

Each emitted transition carries:

    table.insert(transitions, {
        guard = ..., effect = ...,
        reads  = { "done_t1", ... },   -- variables the guard evaluates
        writes = { "active_t2", ... }, -- variables the effect assigns
        internal = true|false,          -- writes only _step_/_done_/_pc_ for its own process
    })

`internal` is the key classification: a transition whose only writes are its own process's
bookkeeping cannot change any property valuation, so it is invisible and a POR candidate.

Source of truth is the AST: `vars_read_by_stmt` and `vars_written_by_stmt` already exist in
`src/codegen/optimize.rs` and can be reused rather than reinvented, provided they are extended
to classify bookkeeping fields as internal.

## What pan actually does (read from the source, not inferred)

Fetched `Src/` from nimble-code/Spin and read the generator:

- **Dependency primitive.** `has_global()` (pangen2.c:2267) answers a single boolean question per
  statement: does it reference any global variable, judged from the `xu` usage masks
  (XS/XR/XX). It is not a per-variable read/write set.
- **How it reaches the generated code.** Each transition is emitted as
  `settr(..., g, tpe0, tpe1)` where `g = (s->frst->status & I_GLOB) ? 1 : 0` (pangen2.c:1248).
  `g == 0` makes the transition LOCAL (`atom |= 8` in the generated `settr`).
- **The reduction itself** is spin's older "safety" machinery: `retrans`, `mark_safety` and the
  `srinc_set`/`srunc` transition classes, all under `#ifndef NOREDUCE` (pangen2.h:554-605).
  This is *conditional safety* reduction, not Peled-style ample sets.

The consequence for this design: our C0-C3 ample-set algorithm is a **different, standard**
sound reduction, not a port of pan's. It is legitimate to choose it, but parity is then an
empirical result to be verified (T3.2), not something the design may assume.

A second consequence, measured: nearly every transition in our models writes a global, so a
"local-transition" reduction of pan's kind would do nothing. The 3906 -> 1370 pan achieves
therefore comes from the conditional-safety classes, and the same class of reduction has to
exist for parity. If C0-C3 alone does not reach it, T2.1 is where that shows up, and the honest
response is to record the shortfall rather than weaken the metric.

## Defect found while implementing T1.1

`emit_assignment` (src/codegen/stmts.rs) prefixed the assignment target with the proctype name
unconditionally, ignoring `global_vars`. A global assigned in a proctype body outside a
`do`/`if` was therefore written to a field that does not exist:

    bool g = 0;
    active proctype P() { g = 1; }
    ltl p0 { [] ( !(g) ) }

    generated: effect = function(s) s.P_g = 1 end
    before:    ours 0 errors, pan 1   (false green)
    after:     ours 1 error, pan 1

Fixed by routing the target through `state_field`, which is the same mapping every other emitter
uses. This is a precondition for T1.1 rather than part of POR: metadata cannot name the right
field if the emitter names a different one, and T1.3 catches exactly that.

## Correction: C2 is not discharged by persistence

This document originally said C2 "for safety is discharged by including all transitions that
are not independent of the set", which describes persistence, not the cycle proviso. That is
wrong, and implementing it as written shipped an unsound reduction in v0.4.0:

    byte tmp; bool bad;
    active proctype A() { do :: (1) -> skip od }
    active proctype B() { do :: (1) -> tmp = 1; bad = 1; break od }
    ltl p { [] ( !bad ) }

    v0.4.0: 2 states, 0 errors.   unreduced and pan: 1 error.

Every state's ample set was persistent, and every state pruned B, because pruning B was locally
justified each time. A loops, so the DFS never returned to the state where B had been dropped.
The corpus contained no looping process, so the gate passed on a wrong algorithm.

The condition that was missing is dynamic, not a property of one state: at any state whose
successor is already on the search path, explore the whole enabled set. Every cycle in the
reduced graph is then broken, the reduced graph is acyclic, and the persistence argument
terminates. Fixed in v0.4.1 (e056815) with two fixtures added to the gate.

Read the C2 bullet below as the reasoning that turned out to be insufficient, not as the design.

## Ample set (T2.1)

Standard persistent-set selection over the enabled transitions of a state, with:

- **C0** if no transition is enabled the ample set is empty;
- **C1** if any enabled transition is visible, the ample set is the whole enabled set;
- **C2** for every non-empty ample set, every cycle in the reduced graph must contain a
  transition from the ample set (the cycle proviso), which for safety is discharged by
  including all transitions that are not independent of the set;
- **C3** the liveness proviso: a cycle may not be entirely within the ample set unless every
  transition of that cycle is itself visible. This is what stops POR from hiding an accepting
  cycle, and it is the condition the dormant code gestures at and does not implement.

Visibility is relative to the properties being checked. Because we evaluate every claim in one
traversal, the visible set is the union of the variables named by all claims. That makes our
visibility strictly coarser than pan's per-claim visibility - which is why the metric is a sum
over pan's per-claim traversals and not a single number.

## The visibility fork (decision needed before T2.1)

Ample-set reduction is bounded by C2: if the ample set is not the whole enabled set, every
transition in it must be **invisible** - it must not change any proposition the property talks
about. Visibility is therefore the binding constraint, and it is relative to which properties
are being checked.

We check every claim in one traversal, so our visible set is the union over all claims. pan
checks one claim per traversal, so its visible set is one claim. On a five-task concurrent model
whose claims each name one task's variables:

    ours (union visible)   3125 states, one traversal
    pan (per-claim)          23 states x 5 traversals = 115

Under the parity metric defined in the proposal (sum over per-claim traversals), pan wins by
~27x **because of our single-pass design**, not because of a missing algorithm. Implementing
C0-C3 on top of a union-visible set would reduce nothing on this shape: nearly every transition
writes a variable some claim mentions.

So there is a fork, and it is architectural rather than algorithmic:

| option | reduction strength | cost |
|---|---|---|
| A. keep one traversal, union visibility | weak on multi-claim models | none; today's design |
| **B. one reduced traversal per claim (CHOSEN)** | matches pan's strength | N traversals internally; public contract preserved by aggregation (below) |
| C. group claims by variable footprint, one traversal per group | between A and B | moderate; grouping is an extra analysis |

The parity metric is satisfied by B almost by construction, and by A only on models where most
transitions are invisible. C is the only option that keeps some single-pass benefit.

### Decision: B, with aggregation

B was chosen. The public contract is nevertheless preserved:

    for each claim c:
        reduced_search(visible = vars(c)) -> violations_c, unverified_c, states_c
    CheckResult { violations: union(violations_c), unverified: union(unverified_c),
                  states_explored: sum(states_c), .. }

One `verify()` call still returns every violation, still attributed by claim name. Two things do
change and must be documented rather than discovered:

- **`states_explored` becomes a sum over per-claim traversals.** That is the honest number and the
  same quantity the parity metric measures, but it is not comparable with the pre-change value,
  and it is not comparable with pan's per-claim `states stored` without the same summing.
- **Model-level findings** (deadlock, unexplored model) must be produced once, on the first
  traversal, not duplicated per claim.

Both are asserted in T2.3.

## Budget across per-claim traversals (decision)

Per-claim traversals raise a question the single traversal never had: what does `max_states`
bound? The caller cannot infer it, so it has to be stated where they read the field.

1. **Per traversal.** `max_states = 200_000` with 40 claims silently becomes eight million states
   of work. Rejected: the caller budgeted a run, not an internal loop they cannot see.
2. **Shared, equal share.** Each claim gets `remaining / claims_left`. Never starves a late claim,
   but truncates a single expensive claim that would have fitted in the whole budget. Rejected:
   it manufactures truncation that need not happen.
3. **Shared, sequential, whole remaining budget (CHOSEN).** Traversals run in declaration order and
   each claim may use whatever budget remains. Total work is bounded by `max_states`; a claim is
   truncated only when the run has genuinely run out, which is precisely what the caller asked
   about.

Field-doc rule:

> `max_states` bounds the whole run, not each claim. Claims are checked in declaration order and
> each may use whatever budget remains. A claim that cannot be decided within what remains is
> reported in `unverified` with kind `StateBudget`, naming that claim - never silently absent.

Starvation is therefore fail-closed per claim: the same invariant as `EarlyStop`, at the
granularity the loop introduced.

### Consequence: declaration order decides coverage

Because the budget is consumed in declaration order, an expensive first claim can exhaust it and
leave every later claim `StateBudget`. A plan with one costly constraint early therefore loses
coverage on cheap constraints late. This is honest and is recorded as a scenario in T2.4, but it is
a real failure mode and the documentation states the rule rather than leaving it to be discovered
as a mysterious unverified entry.

**Alternative considered and deferred: interleaved (round-robin) traversals.** Give each claim a
bounded slice in turn so every claim makes progress before any one exhausts the budget. It removes
the ordering artefact entirely and still bounds total work by `max_states`. Deferred for one
reason: it requires N concurrent DFS stacks and visited sets alive at once, so peak memory becomes
a function of claim count - trading a predictable wall-clock bound for an unpredictable memory
bound. That trade is not obviously right, and it is not needed by the consumer, who accepted
sequential ordering explicitly. Recorded here so a future reader knows it was weighed rather than
missed.

`max_depth` stays per traversal, because depth is a property of a traversal and each claim's
traversal starts at 0. A depth-truncated claim reports `DepthBudget` naming that claim.

`stop_at_first_violation` stops the whole run at the first violated claim; every claim not yet
refuted joins `unverified` with kind `EarlyStop`. That keeps the existing fail-closed behaviour
instead of silently changing what the flag means.

## Gating (T3)

No default change until both hold on the whole corpus:

1. **Verdict equivalence**: for every fixture and every claim, reduced and unreduced searches
   produce identical `violations` and `unverified` (compared by claim name and description
   class, not by count).
2. **Oracle agreement**: reduced error counts equal pan's per-claim counts.

A POR bug that drops a violation produces a green verdict on a broken plan. That is the exact
class the preceding eight changes removed, so the gate is on verdicts, not on state counts.

## Risk

The cycle provisos are where POR implementations are usually wrong, and the error is silent.
The mitigation is not care, it is the two-sided gate: the reduced search must agree with the
unreduced search we already trust on every fixture, and with pan. Any model where the two
disagree is a bug in POR, and the harness must fail rather than pick a winner.

A second risk: emitting dependency metadata by hand for every emission site is error-prone.
T1.3 addresses this by asserting, for every transition in the corpus, that the declared
read/write sets match what the effect closure actually touches, so a missed site fails a test
rather than silently over- or under-approximating dependencies. Over-approximation is safe
(loses reduction); under-approximation is unsound, so the test must catch both.
