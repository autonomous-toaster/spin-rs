# Design

## Temporal categories

| Requirement | Category | Tasks |
|---|---|---|
| Parse Integrity | BEFORE | T1.1, T1.2 before T1.4 |
| Guard Body Classification | CONCURRENTLY | T1.3 alongside T1.1 |
| Verdict Integrity | BEFORE | T2.1, T2.2 before T4.2 |
| Property Evaluation | BEFORE | T3.1 before T3.2 before T3.3 |
| Differential Conformance | BEFORE | T4.1 before T4.2 |

## Root causes found during implementation

The proposal predicted where the defects were. Several turned out to be elsewhere, recorded
here because the final code does not look like the original report described.

### T1.1 - trailing semicolon (the actual D0 cause)

`if_stmt` and `do_stmt` did not consume an optional trailing `;`, unlike every other
statement parser. `fi;` and `od;` are valid Promela, are what veriplan emits, and made the
whole proctype unparsable. Verified by differential parse: `... fi break od` parsed, and real
spin rejects it; `... fi; break od` failed to parse, and real spin accepts it.

### T1.2 - trailing input

`parse()` used `many0(top_level)` and ignored the remainder, so a failed proctype silently
vanished and the model verified with one state. This is what made T1.1 invisible. It also
revealed six codegen tests that had only ever passed vacuously; each is marked `#[ignore]`
naming the parser gap that blocks it.

### T1.3 - guard body classification

Two misclassifications shared one heuristic. A guard body of just `skip` became the
*condition* `skip`; and `ident == expr ->` was read as an assignment because `==` matched
`=`, swallowing the arrow. The veriplan shape uses exactly `:: done_t0_9 == 1 ->`, so this
was a second independent reason no generated plan explored.

### T2.x - three independent verdict defects

1. mlua maps any non-nil, non-boolean value to `true`, while the codegen deliberately emits
   numeric `0`/`1` for boolean expressions. Every guard evaluating to `0` was therefore
   enabled. An assignment guarded by `(a && b)` fired with both operands zero. Fixed at the
   boundary by interpreting Promela truth numerically.
2. `extract_var_from_state` searched for another `:` after the key's own colon, returning the
   *next* key's value. `[](active -> done)` was evaluated using `done`'s value for `active`.
   This produced the false positives on every implication-shaped property.
3. `LTL::parse` sliced `&&` and `||` starting at the preceding space, comparing `" &&"` with
   `"&&"`, so the comparison never matched. Every conjunction collapsed into a single atom
   named `"a && b"`, which did not resolve and was read as false.

### T3.x - atom resolution

`evaluate_atomic_props` reinterpreted the state pointer as a `StateBlob` via `unsafe`. That is
undefined behaviour for any other state type and crashed as soon as a successor state was
evaluated. Resolution now goes through a `Model::atomic_props` method. An atom that does not
resolve aborts the property check with a diagnostic naming it instead of defaulting to false,
and a property that could not be checked is reported as an error rather than logged and
ignored.

### T4.x - the oracle itself

`pan` checks one never claim per run and defaults to the first. `plan_5tasks_3ltls` carries
three properties; `p2` is genuinely violated, yet a default `pan` run reports zero errors
because it never evaluates `p2`. The runner now enumerates named claims, runs `pan -N` for
each, sums errors and takes the widest state count. With that correction, error counts agree
with spin on every corpus model; `plan_5tasks_3ltls` expected_errors was corrected from 0 to
1 because the model really does violate `p2`.

## Remaining divergence

State counts stay higher than spin for multi-process models (25 vs 21, 30 vs 26, 28 vs 14)
because spin-rs materialises per-proctype `_step` counters in the state vector while spin
compresses control locations. `nest_if_one_proc` and `veriplan_block` match exactly.
`plan_20tasks_10ltls` reaches the 1,000,000 state cap against spin's 1,933; that is the same
modelling cost and is performance work, a non-goal here.

## Risk

T1.2 surfaced pre-existing parser gaps (`enabled(P)`, `P[0].y`, `len`/`empty`/`full`,
`ch ? x` in a guard). They are now hard errors rather than silent truncation. Fixing them is
out of scope; the six affected tests are ignored with reasons rather than deleted.

`src/property/buchi.rs` remains an unreconciled duplicate automaton module.
