# Liveness Soundness

## Problem

The product-automaton path returned the opposite verdict for every liveness property, and
two of the three alternative engines returned a verdict without evaluating LTL at all.

Measured against real spin 6.5.2, identical models:

| model | property | truth | spin | spin-rs before |
|---|---|---|---|---|
| `do :: (1) -> p = 0; od` | `<>p` | violated | 1 error | **0 errors** |
| `do :: (1) -> p = 1; od` | `<>p` | holds | 0 errors | **1 error** |
| `x = 1` then terminate | `<>false` | violated | 1 error | **0 errors** |

Three independent causes:

1. `PropertyChecker::check_liveness` built the Büchi automaton for the property itself and
   searched for accepting cycles. An accepting cycle of φ means φ *holds*, so a satisfied
   property was reported as a violation and a violated one was missed.
2. The inner DFS reported any back-edge on its stack as an accepting cycle, without checking
   that the cycle returns to the accepting state that seeded it. A model looping entirely
   inside a rejecting automaton state was reported as a liveness violation.
3. `check_bfs` returned before calling `check_ltl_properties`, so `--search bfs` silently
   skipped every non-propositional LTL property. `--swarm` and `--bfspar` never evaluate LTL
   at all and reported success regardless.

Separately, `--bfspar` deadlocked on every model: each worker held a clone of the frontier
sender and blocked in a blocking receive, so the channel never reported disconnection once
the frontier drained. Its own test suite hung for this reason.

## What changes

- Build the automaton for the negation of the property; complement only the deterministic
  shapes (`p`, `!p`, `[]p`, `<>p`, `Xp`), and fail loudly for anything else.
- Report a cycle only when it returns to the accepting seed.
- Delete the fallback that reported any cycle as a violation. It produced a verdict about
  nothing.
- Call the property check from BFS as well.
- Refuse to run `--swarm` or `--bfspar` on a model that declares LTL, instead of returning a
  clean verdict without evaluating it.
- Terminate parallel BFS on pending/active counters rather than on channel disconnection.

## Non-goals

- No change to which formulas are supported: `U`, `V` and nested temporal operators remain
  unsupported and are now reported as errors rather than mis-verdicts.
- No new LTL operators, no fairness work, no optimisation work.
