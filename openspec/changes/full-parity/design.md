# Design

## Temporal categories

| Requirement | Category | Tasks |
|---|---|---|
| Positioned Enabling | ALWAYS | T2.1, T2.2, T2.7 |
| One Counter Per Proctype | AT MOST ONE | T2.3 |
| Control Transfer Is An Edge | ALWAYS | T2.5, T2.6 |
| Termination Is Derived | ALWAYS | T2.3, T2.5 |
| Folds Retire With Their Cause | AFTER | T2.8 after T2.7 |
| Structural Corpus | ALWAYS | T1.1 |
| Verdict Gate | ALWAYS | T1.2, T1.4 |
| Declines Are Counted | ALWAYS | T1.2 |
| Baseline Before Rewrite | BEFORE | T1.3 before T2.1 |
| Declared Constructs Are Honored | ALWAYS | T6.1, T3.4 |
| Statement Semantics Match pan | ALWAYS | T3.1, T3.2, T3.3 |
| Deadlock Means Invalid End State | ALWAYS | T3.6 |
| Property Surface | ALWAYS | T5.1, T5.2, T5.3 |
| Data Surface | ALWAYS | T4.1, T4.2, T4.3, T4.4, T4.5 |

## What pan does, read from the generator

pan compiles a proctype into a finite state machine at generation time. The state is
`(pc_1 .. pc_n, variables)`; control flow is resolved before anything runs. `goto L` becomes a
pc assignment in an effect; `break` becomes a pc assignment to the loop exit; an `atomic` sequence
becomes one fused transition; a label becomes a pc value, not a transition. There is no
construct that needs its own counter, because there are no constructs left at run time - only
edges.

## What we do today

    ┌────────────────────────────────────────────────────────────────┐
    │ per-proctype state                                              │
    │  _done_<P>   terminated, but also the gadget goto/break use     │
    │  _step_<P>   index inside one guard body                        │
    │  _pc_<P>     written by goto/break; read by label + atomic only  │
    │  _atomic_step_<P>, _unless_step_<P>                             │
    └────────────────────────────────────────────────────────────────┘

Five counters, five conventions, and no single answer to "which statement is enabled next".
The consequence is that composition holds exactly where someone wrote it: inside a guard body
(the step machine) and nowhere else.

## The CFG

Nodes are positions between statements. Edges are statements, each carrying the action it
performs and the enabling condition that action needs beyond its position.

    build:   AST body ──► graph, one node per position, one edge per statement
    emit:    for an edge (from ─► to) carrying statement s:
                 guard  = s._pc_<P> == from  AND  enabling(s)
                 effect = effect(s); s._pc_<P> = to

Termination is a node with no outgoing edges. `_done_ <P>` becomes derived from the pc, and no
control transfer writes it.

| construct | today | CFG |
|---|---|---|
| sequence of statements | unordered | chain of edges |
| `do :: g -> b od` | step machine, owns `_step_` | selection node, one edge per guard, `break` edge to the exit node |
| `if :: g -> b fi` | step machine | selection node, edges join at the continuation |
| `break` | `_done_ = true` and `_pc_` (target always 0) | edge to the loop exit node |
| `goto L` | `_done_ = true` and `_pc_ = L` | edge to L's node |
| `label L` | a transition guarded on `_pc_ == L` | the node itself; no transition |
| `for (i : a..b)` | unordered expansion | init edge, loop-head node, exit edge |
| `unless` | own counter | escape edge to the handler, taken when the body is stuck |
| `atomic` / `d_step` | own counter | one fused edge over the body's edges |
| `assert` | guard, or an unordered effect | edge whose effect records the failure |

## Why not thread one pc through today's emitters

Nesting a `do` inside a sequence needs a return address: either a stack in the state, which adds
failure modes and grows the state vector, or inlining the construct per call site with its own
pc range. Inlining is compiling to a CFG. So the cheap version either does not work or becomes
this design, and it is better to know that before starting.

## What is preserved

The `transitions` table is the contract with everything below the code generator: `guard`, `effect`,
`reads`, `writes`, `internal`, `opaque`, `label`. It does not change shape, so the checker,
reduction, runtime and dependency inference are untouched. That is what makes this a rewiring
rather than a rewrite: the leaf emitters keep their bodies and take `(from, to)` instead of a
hard-coded `guard = true`.

## Consequences that retire special cases

- `qdan`: an assert is an edge, so it is ordered by construction. The failure marker becomes
  a normal write instead of a second semantics bolted onto a guard.
- The hoisted blocking expression: a wait is an edge with a condition, not a statement that must
  be kept out of a guard.
- The trailing-`break` fold: `break` is an edge and needs no fold.
- `nonflat`: refused today; a multi-statement nested branch is just edges.
- Reduction sharpens: `_pc_` writes are already classified internal, so more of every
  transition becomes invisible to POR.

## Risks

- The rewrite has no oracle for `goto`, `label`, `unless`, `for`, `atomic` beyond a handful of hand-written
  models. The generator must cover those constructs **before** the old emitter is deleted,
  otherwise the rewrite is validated only where it was already working.
- Deleting the old emitter removes the fallback. It should be deleted last, after the generator
  gate is green on the new path, not as part of change 1's first commit.
- `_step_` is the only sequencing mechanism that works today. It must not be removed until
  the CFG reproduces every step-machine shape in the corpus.

## The generator

Grammar over body shapes rather than over language features:

| axis | values |
|---|---|
| statement count | 1, 2, 3 |
| statement kind | assignment, assert, send, recv, skip, if, do, for, atomic, unless, goto/label |
| position of the observed write | first, middle, last |
| nesting depth | 0, 1, 2 |
| processes | 1, 2 sharing a variable, 2 not sharing |
| claim | safety over one variable, implication between two, liveness |

Mutation test for the generator itself, in the same spirit as the POR sweep: it MUST be able to
regenerate all three known false refutations - `order`, `x_beforeseq`, `x_twodo` - and
`adv_loop_and_assert`. A generator that cannot reproduce a known defect cannot be trusted to
find an unknown one.

## Human review only

Fairness. pan offers weak fairness (`-f`) and per-proctype `fair` labels. Whether either changes
a verdict on a model we otherwise handle is unknown, and it cannot be formalised as a
requirement until we have a model where the two engines disagree and the disagreement is
attributable to fairness rather than to a defect. Tracked by T3.7 and left out of the gate.
