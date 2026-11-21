# Checker Soundness

## Problem

The verifier reports success for models it never explored. Two independent gaps produce
this, and both were reproduced against real spin 6.5.2 on identical inputs.

**Gap 1 — valid models fail to parse, silently.** `if_stmt` and `do_stmt` do not consume
an optional trailing semicolon, so `fi;` and `od;` are rejected. Every other statement
parser accepts a trailing `;`. Because `parse()` runs `many0(top_level)` and ignores
leftover input, the failure does not surface: the proctype is dropped from the model and
the remaining declarations are returned as if parsing succeeded.

Measured:

| model | spin-rs before | real spin |
|---|---|---|
| m1 no if/fi | 25 states, 0 errors | 15 states, 0 errors |
| m2 `if/fi;` in do, 2 procs | 1 state, **0 transitions**, reports success | 19 states |
| m3 `if/fi;` in do, 1 proc | 1 state, **0 transitions**, reports success | 5 states |

`spin-rs -a m3.pml` emits an empty `_spin_get_transitions()` — no proctype functions at
all. This is a parse/codegen failure presenting as a verification pass.

**Gap 2 — a failed evaluation becomes an empty transition set.** `LuaModel::transitions`
maps a Lua error to `vec![]` after `log::error!`. A guard that raises at runtime is
indistinguishable from a guard that is disabled, so exploration stops at the initial state
and no violation can be found.

## Impact

Downstream (veriplan) generates every plan with an `if :: failed_x = 1; :: skip; fi;`
block, so Gap 1 means no plan has ever had a state explored. Constraints reported as
verified were never checked.

## What changes

1. Parse integrity: accept trailing `;` after `fi`/`od`; reject trailing unparsed input
   rather than silently truncating the model; stop treating keyword-led statements as guard
   conditions.
2. Verdict integrity: a model with at least one proctype and zero explored transitions is an
   error, never a pass; a failed evaluation is an error, not an empty transition set;
   violations are deduplicated per property.
3. Property evaluation: an unresolved atomic proposition is a diagnostic naming the atom,
   never `false`; non-identifier atoms are rejected explicitly; propositions are evaluated
   in the successor state, matching the documented contract.
4. Differential conformance: the bench corpus gains the nested shapes above, and verdict
   comparison uses per-property counts rather than "was any error reported".

## Non-goals

- No new LTL operators; `U`/`V` support is unchanged.
- No fairness changes.
- No partial-order-reduction changes.
- No performance work; state counts may shift because previously-dropped proctypes now run.
- The `src/property/buchi.rs` duplicate automaton module is not reconciled in this change.
