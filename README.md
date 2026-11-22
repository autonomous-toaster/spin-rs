# spin-rs

[![License: WTFPL](https://img.shields.io/badge/License-WTFPL-brightgreen.svg)](http://www.wtfpl.net/)

**A Rust-native Promela model checker with Lua runtime — no GCC dependency required.**

Parse, compile, and verify Promela models entirely in-process. `spin-rs` translates Promela to
Lua, then executes transitions via `mlua` (embedded Lua 5.4). No C compiler, no subprocesses.

## Quick Start

```bash
# Install
git clone https://github.com/autonomous-toaster/spin-rs.git
cd spin-rs
cargo build --release

# Verify a model
./target/release/spin-rs examples/model.pml

# Interactive simulation (step through your model)
./target/release/spin-rs -i examples/model.pml
```

## Usage Examples

### Basic Verification

```bash
# Verify a Promela model (DFS, exact storage)
spin-rs model.pml

# BFS search (finds shortest counterexample)
spin-rs --search bfs model.pml

# Limit state space
spin-rs --max-states 100000 --max-depth 1000 model.pml
```

### Interactive Simulation

Step through your model transition by transition, like Spin's `-i` mode:

```bash
spin-rs -i model.pml
```

Commands during simulation:

- `<N>` — take transition N
- `b` — step back (undo)
- `i` — inspect current state variables
- `h` — show history
- `q` — quit

### LTL Property Verification

```bash
# Verify a liveness property
spin-rs --ltl liveness '[]<>(x == 0)' model.pml

# Multiple LTL formulas can be inlined in the Promela source
```

### Trail Replay

```bash
# Replay an error trail with state inspection
spin-rs -t -k error.trail model.pml

# With full state dumps at each step
spin-rs -t -k error.trail --inspect model.pml
```

### Optimization Levels

```bash
# Dead variable elimination (removes unused vars from state vector)
spin-rs -o2 model.pml

# Statement merging (combines consecutive deterministic transitions)
spin-rs --opt3 model.pml

# Rendezvous optimization (merges sync send/recv pairs)
spin-rs --opt4 model.pml
```

### Advanced Verification Modes

```bash
# Swarm verification: N parallel workers with varied parameters
spin-rs --swarm 8,1 model.pml

# Parallel BFS with N threads
spin-rs --bfspar 4 model.pml

# Hash-compact storage (memory-efficient, 64-bit hashes)
spin-rs --hc model.pml

# Strong fairness constraints for liveness
spin-rs --strong-fairness model.pml

# Bitstate hashing (Bloom filter, approximate)
spin-rs --storage bitstate model.pml

# Partial order reduction is on by default; turn it off to explore everything
spin-rs model.pml
spin-rs --no-por model.pml
```

## Compatibility with spin and pan

Nothing here is claimed without a measurement. Verdicts are compared with real `spin`/`pan` on
two corpora, and the gate fails on a wrong verdict:

- **158 generated models** (`just parity`), built from a grammar of body shapes: statement order,
  nesting, fusion, loop exit, channels, two-process races, and seven claims per body.
- **the 78 models spin itself ships** under `Examples/` (`parity_gate --dir <spin>/Examples`).
  Spin has no test suite; those are the closest thing, and their author wrote pan.

Both currently report **zero wrong verdicts**. That sentence is only meaningful alongside the
categories below, because this engine answers three ways and only one of them is a verdict:

| outcome | meaning | counts as agreement? |
|---|---|---|
| decided | a verdict and an error count, which must equal pan's | that is the comparison |
| declined | no verdict; a named `UnverifiedKind` says why | never |
| refused | the model cannot be run at all (hard error) | never |

### Supported, checked against pan

- **Statements**: assignment (with the declared type's truncation), `if :: else fi`,
  `do :: else od`, `for (i : a .. b)`, `break`, `goto` and labels, `atomic`/`d_step` fused into one
  transition, `assert`, `printf`, `skip`.
- **Control flow semantics**: a `do` whose guards run out *blocks* (pan's invalid end state), a
  `for` leaves its range, and every statement of a body is its own step - a `for` body is not
  atomic, and neither is anything else.
- **Types**: `bit`, `bool`, `byte`, `short`, `int`, `unsigned(n)`, `mtype`; values are truncated to
  the declared type on assignment (`byte x = 255; x = x + 1` leaves 0); `/` and `%` follow C, so
  `-7 % 2` is -1; a guard is true when it is non-zero, so `(n)` with `n == 0` is false.
- **Channels**: buffered (`[N]`), arrays (`chan c[N] = [cap] of { t }`), send and receive as
  statements and as guards, `len`/`full`/`empty`/`nfull`/`nempty`. A channel's queue is part of the
  state, so a state always determines its own transitions.
- **Proctypes**: several instances (`active [N]`), an `init` block, `provided (expr)`.
- **Preprocessor**: object-like `#define`, including a macro that names another macro.
- **Claims**: `[]p` over propositions, `<>p`, `->`, `<->`, and atoms that are expressions -
  comparisons, `&&`, `||`, `!` - evaluated against the state. One claim per file is compared with
  pan in the corpus; several claims are checked in one run here and summed per claim there.

### Refused: the model runs nowhere

These are hard errors. No verdict is produced, so nothing about the model is claimed.

| construct | why |
|---|---|
| rendezvous channel (`chan c = [0] of { t }`) | the send and its receive are one transition over two processes; this engine moves one process per transition |
| `run P(...)` | a spawned process needs dynamic instances; the transition table is emitted per static instance |
| `unless`, and the bare `{ ... }` block it is written with | neither parses: a bare block is not a statement the parser has, and `unless` needs one. The refusal is a parse error naming the offset, not a refusal naming the construct |

| `priority N` | pan runs only the highest-priority enabled process; ignoring it refuted a model pan accepts, so it is refused instead |
| typedef records (`r.f`) | structured fields are not in the state |
| `P@x` (remote variable) | the reference does not resolve |
| `pc_value(pid)` | it exposes the checker's own statement numbering, which is not comparable with pan's |
| `#include`, function-like macros, `#if`/`#ifdef` | not implemented, and expanding them wrongly would produce a different model |

### Declined: no verdict, but a named reason

- `[]<>p`, `<>[]p`, nested temporal operators, and `U`/`V`. The translation refuses rather than
  approximating; `[]<>p` is *not* treated as `[]p`.
- An atom that names no state variable, reported as `unsupported_property`, and an atom that is not
  even an expression, reported as `unparsable_formula` naming the fragment. `P[0]:x` (a remote
  local) lands here.
- A `never` claim. It is generated, but the claim process is in no state and is dispatched by
  nothing, so a model carrying one is reported as unchecked - `never { skip }` is a violation in
  pan and would otherwise come back "verified".

### Where this engine deliberately differs

- **State counts are not comparable and are not a goal.** Claims are checked in separate
  reduced traversals, so `states_explored` is a sum over claims, and the reduction is a different
  algorithm from pan's. The contract is the verdict and the error count per claim.
- **Speed.** Measured by a consumer on a 62-claim plan: about 2.2 ms per state, so 200 000 states
  per claim is minutes of wall clock. pan's C engine is much faster per state. If a plan needs a
  budget that large, this engine will be slow whatever the verdict.
- **`max_states` is shared out.** Each claim gets `max_states / claims` (at least one state), and
  what a decided claim does not use is handed to the claims behind it. An expensive first claim
  cannot take the pool. A starved claim reports `states_used` and `budget`, so a caller can size
  the number from what the run measured instead of guessing.

### Reproducing any of this

```bash
just parity                                   # generated corpus, fails on a wrong verdict
cargo run --release --bin parity_gate -- --dir /path/to/Spin/Examples
```

### Partial Order Reduction

Reduction is on by default. It emits per-transition read/write sets from the AST, then explores
only an ample set at each state: if no enabled transition can affect the claim being checked, one
transition that conflicts with nothing else is enough, because the others commute with it.

Two consequences of doing this per claim, which the field documentation states as well:

- **`states_explored` is a sum over per-claim traversals.** A model with two claims may report
  twice the states of a single unreduced pass. It is the same quantity as summing pan's per-claim
  runs, and it is not comparable with the pre-reduction value of the field.
- **`--stop-at-first-violation` stops each claim's traversal**, not the run: a refutation is cheap,
  every other claim still gets its turn, and the share a decided claim frees goes to the claims
  behind it. The liveness pass is skipped in that mode, and every claim it would have examined is
  reported as `early_stop` with a reason saying it was never examined - it is not reported as
  satisfied, which is what an absent claim would otherwise mean.
- **`max_states` is shared out between the claims.** Each claim gets `max_states / claims`, at
  least one state, and whatever a *decided* claim does not use is handed to the claims behind it -
  an undecided claim keeps nothing, so an expensive early claim cannot take the pool. A starved
  claim appears in `unverified` with kind `state_budget` naming it, and carries `states_used` and
  `budget` so a caller can size the number from what the run measured rather than guessing.

Liveness claims are checked by the nested DFS, which is never reduced: an accepting cycle can
consist entirely of invisible transitions, so reducing it would hide the violation.

#### Parity against pan

States stored. "pan POR" is the sum over pan's per-claim traversals; that is the only form
comparable with our aggregated count. Every fixture here declares at least one claim.

| fixture | claims | ours (reduced) | pan POR | pan `-DNOREDUCE` | errors | pan errors |
|---|---|---|---|---|---|---|
| 5 independent proctypes, claim over two of them | 1 | 20 | 1029 | 3125 | 0 | 0 |
| 4-task dependent chain | 2 | 18 | 26 | 26 | 0 | 0 |
| concurrent two-phase barrier | 2 | 20 | 26 | 34 | 0 | 0 |
| veriplan chain shape | 1 | 5 | 7 | 7 | 0 | 0 |
| veriplan chain shape | 2 | 14 | 20 | 20 | 0 | 0 |
| violation behind a dependent guard | 1 | 5 | 5 | 5 | 1 | 1 |
| violated liveness claim over an internal cycle | 1 | 2 | 2 | 2 | 1 | 1 |

Deliberately not in that table: the same five independent proctypes with **no claim**, where we
explore 21 states and pan 1370. With no property, both engines are only measuring how small a
deadlock-preserving reduction they can get away with, and ours commutes far more aggressively
than pan's conditional-safety classes. Quoting that beside a claim-bearing row invites a
factor-of-65 comparison that means nothing. (Raised by the consumer; they were right.)

#### The cycle proviso, and the false green that shipped without it

v0.4.0 was unsound. This model reported 2 states and 0 errors; the unreduced search and pan
both report 1 error:

```promela
byte tmp; bool bad;
active proctype A() { do :: (1) -> skip od }
active proctype B() { do :: (1) -> tmp = 1; bad = 1; break od }
ltl p { [] ( !bad ) }
```

The ample set was persistent at every state - dropping `B`'s transition was locally justified
because it commutes with `A`'s - but `A` loops, so every state kept only its own transition and
the DFS never returned to the state where `B` had been dropped. `bad` was never set.

Every cycle in the reduced graph must be broken by exploring everything: a state whose successor
is already on the search path explores its whole enabled set, which makes the reduced graph
acyclic and lets the persistence argument terminate instead of going round. Two fixtures now
pin this (`cycle_hidden_violation`, plus a loop beside a dependent guard).

The same review found the opposite error: a *visible* but **independent** enabled transition was
forcing the whole set to be explored. That is stricter than the ample-set condition, which only
requires the picked transition to be invisible and independent of everything else - a visible one
that is independent stays enabled and is taken later. Relaxing it took the claim-bearing
five-process fixture from 2501 states to 30.
