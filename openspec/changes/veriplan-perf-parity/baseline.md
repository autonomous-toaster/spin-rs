# Baseline: the consumer's chain shape

Recorded before any change, with `cargo run --release --bin parity_gate -- --chain` at
v0.5.35 (`b31e386`). pan's numbers are re-derived by the run, not vendored.

## The two directions, and why both are recorded

| fixture | direction | ours (errors) | pan states | pan errors | our elapsed |
|---|---|---|---|---|---|
| N=6 | refuting | 1 | 3 | 1 | 0.052s |
| N=6 | closing | 0 | 37 | 0 | 0.068s |
| N=8 | refuting | 1 | 3 | 1 | 0.244s |
| N=8 | closing | 0 | 49 | 0 | 0.339s |
| N=10 | refuting | 1 | 3 | 1 | 1.166s |
| N=10 | closing | 0 | 61 | 0 | 1.592s |
| N=12 | refuting | 1 | 3 | 1 | 5.029s |
| N=12 | closing | 0 | 73 | 0 | 7.268s |

**The verdicts already agree in both directions.** That is the whole reason this instrument
exists: a gate that reads only the error count reports full parity here, on a model where the
state count is 2^N against pan's linear.

## Our state counts, which the table above does not carry

The table records verdicts because that is what was compared against pan. Our own state counts,
measured separately on the same fixtures:

| N | refuting | closing |
|---|---|---|
| 6 | 694 | 694 |
| 8 | 2806 | 2806 |
| 10 | 11254 | 11254 |
| 12 | 45046 | 45046 |

Identical in both directions, because this search does not stop early. Two consequences:

1. A matching count on the refuting fixture is evidence about neither projection nor parity - it
   says only that the whole space was explored.
2. A swapped fixture is invisible in our own output, which is why the direction is pinned by a
   test rather than a comment.

## Growth

Ours: 694 -> 2806 -> 11254 -> 45046 is x4.04, x4.01, x4.00 per two tasks = **x2 per task**.
pan: 37 -> 49 -> 61 -> 73 is **linear, ~6N + 1**.

Per-claim states against pan at the same N: ~19x at N=6, ~57x at N=8, ~184x at N=10, ~617x at
N=12. The ratio is what doubles per task, so a constant-factor criterion is only meaningful
when it is stated at a fixed N and checked across N for growth.

## Wall clock

0.068s at N=6 to 7.268s at N=12 for the closing probe: ~6.3k states/s. pan does 1e5-1e6. This
is the a6fr baseline; nothing in phases 2-3 is expected to move it.
