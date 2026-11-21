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

### Partial Order Reduction

Reduction is on by default. It emits per-transition read/write sets from the AST, then explores
only an ample set at each state: if no enabled transition can affect the claim being checked, one
transition that conflicts with nothing else is enough, because the others commute with it.

Two consequences of doing this per claim, which the field documentation states as well:

- **`states_explored` is a sum over per-claim traversals.** A model with two claims may report
  twice the states of a single unreduced pass. It is the same quantity as summing pan's per-claim
  runs, and it is not comparable with the pre-reduction value of the field.
- **`max_states` bounds the whole run, not each traversal.** Claims are taken in declaration
  order and each may use whatever budget remains, so an expensive early claim can starve later
  ones. A starved claim appears in `unverified` with kind `state_budget` naming it.

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
