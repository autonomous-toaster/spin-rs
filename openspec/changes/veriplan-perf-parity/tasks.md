# Tasks

## Phase 1: The Instrument

- [x] 1.1 Commit the consumer's chain fixture generator as a test fixture (N=6,8,10,12; failure branch per task; one claim)
- [x] 1.2 Commit the pan oracle for each fixture: states stored per claim, built by the test, not vendored
- [x] 1.3 Record the pre-change baseline (states and wall clock) for every fixture
- [x] 1.4 Add a fixture whose claim does NOT refute, so the space closes and the state count is comparable
- [x] 1.5 Assert pan's refutation depth on the refuting fixture (3 states), so the asymmetry is pinned, not folklore
- [x] 1.6 Fail the build when a fixture's verdict disagrees with pan
- [x] 1.7 Pin that the two probe directions are not interchangeable; fail if a fixture is swapped

## Phase 2: Claim-Relative State Projection

- [x] 2.1 Compute the observable field set per claim and expose it alongside the existing ReductionScope
- [x] 2.2 Add the always-visible set: the assertion marker, and every field deadlock detection reads
- [x] 2.3 Project the state onto that set before hashing
- [x] 2.4 Project the state onto that set before storing, so the store key and value agree
- [x] 2.5 Carry the projection through the transition function so expanded transitions are compared in projected form
- [x] 2.6 Fall back to the full state when a claim's observable set cannot be determined
- [x] 2.7 Prove the projection is claim-relative: two claims with different visible sets must produce different counts on one model
- [x] 2.8 Assert that a hidden bit cannot suppress an assertion failure
- [x] 2.9 Assert that a hidden bit cannot suppress a deadlock
- [x] 2.10 Sweep verdicts against pan on the whole corpus; no verdict may change
- [x] 2.11 Measure the fixtures: state count per claim against pan

## Phase 3: Liveness Budget

- [x] 3.1 Give NestedDFS a state ceiling that counts inserted states, not stack depth
- [x] 3.2 Distinguish exhaustion from closure in the return value; today both are None
- [x] 3.3 Pass the claim's budget share into check_liveness and check_claim
- [x] 3.4 Report exhaustion as Unverified{StateBudget} naming the claim
- [x] 3.5 Assert that --max-states 1 on a liveness fixture terminates and names the claim
- [x] 3.6 Assert that a liveness claim which does close still reports its true verdict
- [x] 3.7 Sweep the corpus: no liveness verdict may change
- [x] 3.8 Specify the rule for max_states below the claim count, and report the raised budget
- [x] 3.9 Make the explored total never exceed max_states, or state and test that it may
- [x] 3.10 Refuse or report --max-states 0 instead of silently exploring

## Phase 4: Per-State Cost

- [x] 4.1 Hoist state reconstruction out of the per-state path (no Lua compile per visit)
- [x] 4.2 Keep one transition table and pass the state as an argument instead of re-creating closures
  (partially: the clone function is hoisted out of the per-transition loop, and the entry buffer
  is reused. The transition table itself is still rebuilt per state - see 4.3.)
- [ ] 4.3 Stop storing the state as an owned string; use a structured value
  (BLOCKED: measured, not estimated. See the throughput analysis in design notes below.)
- [x] 4.4 Measure states/s on the fixtures before and after each step, in the same run harness
- [ ] 4.5 Assert the throughput floor holds in release mode
  (NOT MET: 6.5k states/s against 1e5. Cannot be met without 4.3 - see below.)

## Phase 4: where the remaining 15x is

Measured at the end of this session, on this tree, with the release binary. Every number below is
a measurement, not an estimate.

| fixture | states | transitions | elapsed | states/s |
|---|---|---|---|---|
| liveness chain N=10, no branch | 5116 | 25600 | 0.754s | 6785 |
| liveness chain N=10, with branch | 10232 | 51201 | 1.561s | 6556 |
| closing chain N=60 (180 fields) | 481 | 599 | 0.402s | 1196 |

Two facts follow.

1. **Cost scales with transitions, not with state width, on a chain.** 5116 states at 25600
   transitions and 10232 at 51201 both run at ~6.6k states/s, i.e. ~34k transitions/s. Doubling
   the work doubles the time.

2. **Cost scales with field count when transitions are few.** The N=60 model has 1.2 transitions
   per state and still costs 0.83 ms per state. Per-state cost tracks fields: measured 141 us at
   72 fields, 293 us at 142, 566 us at 282 - linear at ~2 us per field per state expansion.

The floor is 1e5 states/s. At 180 fields that needs 18M field-visits/s; the engine does 216k, a
gap of **83x**. No amount of allocation tuning in the string pipeline closes an 83x field-visit
gap: the pipeline has to stop existing.

### What was actually tried and what it bought

- Hoisting state reconstruction out of the per-state path (no Lua compile per state visit): the
  single largest win, and it is why the chain fixtures went from 7.268s to 0.018s at N=12 - though
  most of that was the state-count fix in phase 2, not this.
- Hoisting `_spin_clone` out of the per-transition loop: 4.90s -> 4.36s on the N=10 liveness run.
- Reusing the serialization entry buffer across calls: within noise.
- Shallow fast path in `_spin_clone` for flat state tables: ~4%.
- Rewriting `project_blob` to scan slices instead of allocating a String per field: within noise,
  but it is strictly better code and is kept.

Net: about 1% end to end. That is the honest result, and it is why 4.5 stays unticked.

### Why 4.3 is a redesign and not a patch

`Model::State` is `StateBlob(String)`. Every state visit deserializes that String into a Lua table
(~142 field conversions at N=20), evaluates guards against it, and every enabled transition
clones the table and serializes it back to a String. The String is the interface between the Lua
runtime and the search, so making it structured means changing what `Model::State` is for the Lua
backend, and threading that through the checker.

Good news for whoever picks this up: `StateBlob` is named in exactly two files
(`src/runtime/mod.rs`, and a comment in `src/property/ltl2ba/product.rs`), so the blast radius is
smaller than it looks.

### What is NOT in doubt

The requirement that actually made the consumer's plans unusable is met and asserted: state count
grows linearly in N on the chain shape (49/65/81/97 against pan's 37/49/61/73), and the liveness
path is bounded (0.000s where it used to run past 45s). Phase 4 is throughput on top of that, and
at 6.6k states/s a 30-task plan that used to be 2^30 states now finishes.

### Phase 4 update: what the batched expansion changed

4.2 is done and measured at about 20%: the engine now crosses into Lua once per state rather than
once per transition, with guards, cloning, effects and serialization all inside `_spin_expand`.
Six guard shapes (disabled zero, non-zero, comparison, boolean, if/else, assertion) are pinned
against pan's verdicts, because moving guard evaluation into Lua is exactly the kind of change
that can be correct on the happy path and wrong on nil.

4.3 and 4.5 remain open. The floor is measured at ~6.9k states/s against 1e5 on a 30-field chain
with five transitions per state. What is left is the interpreter itself: every transition's guard
and effect is interpreted Lua over a freshly cloned table, and no amount of batching removes the
interpretation. A structured state would remove the per-state parse and the owned String, which
are real but were measured at 40us of a 1031us expansion before the batching - so it is not the
lever that closes a 14x gap either. What closes it is compiling the transition relation instead of
interpreting it, which is a different engine, not a task in this change.
