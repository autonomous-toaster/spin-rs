# Client Defects S1-S4

## Problem

Reported by the veriplan client against v0.1.1 (1933fc2), and reproduced.

**S1 — a violated top-level `<>` reports success.** A terminating model has no cycle, so a
product search that only reports cycles it finds can never refute `[]!p`. Every violated
`<>p` over a finite run came back green, and so did `<>(false)`, while `pan` refutes both.

**S4 — an unsupported property is counted as a violation.** `[](a -> <> b)` is the shape
veriplan emits for every conditional constraint. When `a` never fires the property HOLDS,
yet the checker could not complement the shape and reported one error, so a satisfied
property read as invalid.

**S2 — the README teaches a false green**, using `[]<>(x == 0)` as the `verify_ltl` example.

**S3 — the manifest disagrees with the tag**: `version = "0.1.0"` under tag v0.1.1, and a
behaviour-changing release was numbered as a patch.

## What changes

1. Model claim stuttering at terminal states, so a finite run can refute a `<>` claim.
2. Complement `[](a -> <> b)` as `<>(a && []!b)`: two states, accepting once the
   antecedent fired and the consequent has not arrived.
3. Stop counting an undecidable property as a violation. `CheckResult` gains
   `unverified: Vec<Unverified>`, which also records a search truncated at `max_states`,
   so "closed, clean" is distinguishable from "gave up" and from "invalid".
4. The CLI reports an inconclusive run instead of printing success.
5. README documents the three outcomes, the `Err` conditions, the zero-transition rule, the
   absent deadline, and why 0.1.0 verdicts were wrong. The `U`/`V` operators are no longer
   advertised.
6. Manifest version 0.2.0, released as v0.2.0: `verify` returning `Err` where it returned
   `Ok` is a break for callers.

## Non-goals

- No new LTL operators. `U`, `V` and nested temporal operators remain unsupported and are now
  reported as unverified rather than mis-verdicted.
- No fairness or performance work.
