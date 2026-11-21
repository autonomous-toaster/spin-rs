# Violation Attribution

## Problem

A refuted liveness claim was reported with `property_name = "LTL"`, so a per-property caller
could not attribute it. Consumers read a claim index out of the name; `"LTL"` parses to none,
so the violation was dropped and the violated `[] (a -> <> b)` requirement was counted as
satisfied. It failed safe only because the consumer's canary refuses to certify an unnamed
claim.

While reproducing it, a second defect surfaced in the same automaton: the complement of
`[] (a -> <> b)` returned to its waiting state when the consequent arrived, so it could
re-guess the witness point. A run where the consequent keeps arriving — a property that
HOLDS — was accepted as a violation. On the consumer's three-claim canary this reported
`p2` refuted and missed `p1`, the exact opposite of `pan` (`p0 1, p1 1, p2 0`).

## What changes

- `NestedDFS` carries the claim name and reports it on a violation.
- Model-level violations are named `deadlock`, `model` or `assertion`, none of which can be
  mistaken for a claim.
- The witness automaton gains a dead state that absorbs the run once the consequent
  arrives, so its accepting loop is reachable only while the consequent stays away.
- Its guards are general propositional formulas in DNF, so an unsatisfiable obligation
  (`x && !x`) is refuted rather than reported as unsupported.
- The CLI prints undecided properties even when there are also violations.

## Non-goals

- No change to which operators are supported.
- No fairness or performance work.
