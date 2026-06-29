## Why

spin-rs generates Lua from Promela, but several core control-flow and channel constructs produce incorrect or missing transitions. The generated verifier can silently skip states, mis-handle non-deterministic choice, or fail to detect liveness violations. These are not edge cases — they affect every model with goto, break, unless, atomic/d_step, or non-trivial channel operations.

The original Spin C verifier handles these correctly through explicit transition tables and careful state-machine generation. spin-rs must match that correctness to be a viable replacement.

## What Changes

### 1. Goto / Break / Label Handling

Current codegen emits `-- goto X` comments and `-- break` comments — no actual transitions. Labels are parsed but produce no code. This means:

- `goto` targets are unreachable (dead code after goto is still generated)
- `break` never exits loops
- Labels used as jump targets produce no state

**Fix**: Track label positions during codegen, emit transitions that set the program counter (step variable) to the target label's position. Break sets the step to exit the enclosing do-loop.

### 2. Atomic / D-Step Semantics

Current codegen combines all guards and effects into a single transition. This is incorrect for:

- `atomic { ... }` — each inner statement must be individually executable; if any guard fails mid-sequence, the entire atomic block must be retried from the start
- `d_step { ... }` — same as atomic but no intermediate states are stored (deterministic)

**Fix**: Generate a state machine for atomic/d_step blocks: entry state checks the first guard, each inner transition advances to the next, and any failure resets to the entry.

### 3. Channel Operations (Sorted Send, Random Recv, Poll, Eval)

Current codegen handles only basic `!` (send) and `?` (receive) with variable lists. Missing:

- `!!` sorted send — insert message in sorted order
- `??` random receive — non-deterministically pick any matching message
- `?<expr>` poll receive — check without consuming
- `eval(expr)` in receive — match against a specific value

**Fix**: Add Rust-side channel operations for sorted send, random recv, poll, and eval matching. Expose via FFI to Lua.

### 4. Unless Statement

Current codegen has `-- unless (TODO)`. The `unless` construct allows an escape sequence that can interrupt the main body when its guard becomes enabled.

**Fix**: Generate a state machine where at each step of the main body, the unless guard is also evaluated. If the unless guard becomes enabled, control transfers to the unless handler.

### 5. Acceptance Cycle Detection

Current nested DFS checks for any cycle containing an accepting state (Büchi acceptance). This is needed for liveness properties (`[]<>p`, `<>[]p`). The current implementation only checks for basic reachability of violation states.

**Fix**: Implement proper acceptance cycle detection in the nested DFS: track which states are accepting in the Büchi automaton, and during the second DFS, only follow paths that lead back to an accepting state on the stack.

### 6. Non-Progress Cycle Detection

Spin supports `np_` (non-progress) to detect cycles where no progress label is visited. This requires tracking progress labels and checking for cycles that avoid them.

**Fix**: Add progress label tracking to the state vector. During DFS, mark states that contain progress labels. Run a second DFS to find cycles that never visit a progress-marked state.

## Capabilities

### New Capabilities

- `goto-break-label`: Promela goto, break, and label statements produce correct transitions in generated Lua
- `atomic-dstep`: Atomic and d_step blocks execute with correct state-machine semantics
- `channel-ops`: Sorted send, random receive, poll receive, and eval matching work correctly
- `unless`: Unless statements produce correct escape transitions
- `acceptance-cycles`: Nested DFS detects acceptance cycles for liveness properties
- `nonprogress-cycles`: Non-progress cycle detection works with np_ labels

### Modified Capabilities

- `ltl-verification`: Now detects acceptance cycles (not just safety violations)
- `codegen-core`: All control-flow constructs produce correct Lua transitions
