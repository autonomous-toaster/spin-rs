# Nested If Codegen

## Problem

An `if/fi` nested inside a `do/od` body was silently discarded by guard codegen.

When a guard body needs the step machine, each body statement is emitted by a match on the
statement. `Stmt::If` and `Stmt::Do` had no arm and fell through to the catch-all, which
emitted only "advance the step counter". The nested block produced no transition at all, so an
assignment inside it never happened.

Measured against `pan`, property `[] ( !(f) )`:

| model | pan | before | after |
|---|---|---|---|
| `do :: (1) -> f = 1; break od` | 1 | 1 | 1 |
| `if :: f = 1; :: skip; fi;` top level | 1 | 1 | 1 |
| `do :: (1) -> if :: f = 1; :: skip; fi; break od` | 1 | **0** | 1 |

This is the same class as the earlier parse-side defect but a different site: previously
`fi;` made the proctype unparsable, here the block parses and is then dropped.

## What changes

- A nested `if` expands into one transition per branch from the current step, continuing at the
  following step, matching what the flat emitter produces for a top-level `if`.
- A branch that cannot be expressed in a single step, and a nested `do/od` which needs its own
  selection loop, emit a transition that raises at evaluation time, so the run reports
  "could not be checked" rather than silently losing the block.

## Non-goals

- No general nested-`do` support. It now fails loudly instead of silently.
- No optimisation or performance work.
