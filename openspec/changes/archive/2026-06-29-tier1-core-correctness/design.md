## Context

The Lua codegen in `src/codegen/` emits per-statement transitions as Lua closures. Each transition has a `guard` function and an `effect` function. The engine calls `transitions(state)` to get all enabled transitions, then picks one non-deterministically.

The current approach of "one transition per statement" breaks down for goto, break, atomic, d_step, and unless because these constructs require state-machine semantics (multiple states per statement, conditional transitions between them).

## Goals / Non-Goals

**Goals:**

- All Promela control-flow constructs produce correct state exploration
- Liveness properties (acceptance cycles, non-progress cycles) are detected
- All existing tests continue to pass
- Generated Lua is still human-readable

**Non-Goals:**

- Performance optimization of the generated state machines
- Full Spin-compatible trail format for all violation types
- Interactive simulation mode

## Decisions

**Decision 1: Step-variable state machine for goto/break/label**

Each proctype gets a `_step` variable in the state vector. Labels are assigned sequential step numbers during codegen. Goto emits a transition that sets `_step` to the target label's number. Break sets `_step` to the exit step of the enclosing do-loop. The transition enumerator checks `_step` to decide which transitions are enabled.

This mirrors how Spin's C verifier uses a program counter (pc) per process.

**Decision 2: Multi-state expansion for atomic/d_step**

Instead of combining all guards into one, expand atomic/d_step into a sequence of states:

- State 0: check first inner guard, advance to state 1 on success
- State N: check Nth inner guard, advance to state N+1 on success
- Final state: reset to 0 (atomic completed)
- Any guard failure: reset to 0 (retry entire block)

For d_step, intermediate states are not stored in the visited set (they are transient).

**Decision 3: Rust-side channel operations with sorted/random/poll/eval**

Add new channel operation variants in `src/runtime/channel.rs`:

- `chan_send_sorted(channel, value)` — insert in sorted order
- `chan_recv_random(channel)` — non-deterministically pick any message
- `chan_poll(channel, expr)` — check if message matches without consuming
- `chan_recv_eval(channel, value)` — receive only if first message matches value

Expose these via Lua FFI functions alongside existing `chan_send`/`chan_recv`.

**Decision 4: Unless as state-machine expansion**

Expand `unless` at codegen time: for each step in the main body, also emit a transition that checks the unless guard. If the unless guard is enabled, the effect transfers control to the unless handler body. The unless handler runs once and then the process terminates (or continues, depending on context).

**Decision 5: Acceptance cycle detection via second DFS**

The nested DFS algorithm:

1. First DFS: explore all states, mark accepting states (from Büchi automaton)
2. Second DFS: for each accepting state found in first DFS, start a new DFS that only follows paths staying within accepting states. If it finds a state already on the second DFS stack, an acceptance cycle is detected.

This is the standard Spin algorithm for emptiness checking.

**Decision 6: Non-progress cycle detection**

Add a `_progress` bit to the state vector. When a transition visits a progress label, set the bit. During DFS, track whether the current path has visited a progress label. If a cycle is found with no progress label visited, report a non-progress cycle violation.

## Risks / Trade-offs

- **Step-variable overhead**: Each proctype gets an additional `_step` variable, increasing state vector size slightly
- **Atomic expansion**: Multi-state atomic blocks increase the state space (more intermediate states). This is correct but may be slower
- **Channel complexity**: Sorted send and random recv require O(n) operations on the channel buffer, which is fine for typical model sizes
- **Unless correctness**: The expansion approach is correct for Promela semantics but may generate many transitions for deeply nested unless
