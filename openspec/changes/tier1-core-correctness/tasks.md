## Phase 1: Goto / Break / Label

- [x] 1.1 Assign sequential step numbers to all labels in a proctype body during codegen
- [x] 1.2 Emit `_step` variable in state vector for each proctype
- [x] 1.3 Emit transitions for `goto label`: guard checks `_step == current`, effect sets `_step = target`
- [x] 1.4 Emit transitions for `break`: effect sets `_step = exit_step` of enclosing do-loop
- [x] 1.5 Emit transitions for labels: guard checks `_step == label_step`, effect is the label's body
- [ ] 1.6 Add test: goto within a single proctype produces correct state sequence
- [ ] 1.7 Add test: break exits do-loop correctly
- [ ] 1.8 Add test: label as goto target is reachable

## Phase 2: Atomic / D-Step

- [ ] 2.1 Design state machine expansion for atomic blocks: N states for N inner statements
- [ ] 2.2 Implement atomic expansion in codegen: entry state, N intermediate states, reset state
- [ ] 2.3 Implement d_step expansion: same as atomic but intermediate states are transient (not stored)
- [ ] 2.4 Add test: atomic block with failing guard retries from start
- [ ] 2.5 Add test: d_step block produces no intermediate states in visited set
- [ ] 2.6 Add test: nested atomic inside do-loop

## Phase 3: Channel Operations

- [ ] 3.1 Add `chan_send_sorted` to runtime: insert message in sorted order in channel buffer
- [ ] 3.2 Add `chan_recv_random` to runtime: non-deterministically pick any matching message
- [ ] 3.3 Add `chan_poll` to runtime: check message match without consuming
- [ ] 3.4 Add `chan_recv_eval` to runtime: receive only if first message matches given value
- [ ] 3.5 Expose new channel ops as Lua FFI functions
- [ ] 3.6 Update codegen to emit `!!` as sorted send
- [ ] 3.7 Update codegen to emit `??` as random receive
- [ ] 3.8 Update codegen to emit `?<expr>` as poll receive
- [ ] 3.9 Update codegen to emit `eval(expr)` in receive as eval recv
- [ ] 3.10 Add test: sorted send maintains order
- [ ] 3.11 Add test: random receive picks different messages across runs
- [ ] 3.12 Add test: poll receive does not consume message
- [ ] 3.13 Add test: eval receive matches specific value

## Phase 4: Unless

- [ ] 4.1 Design unless expansion: for each main-body step, emit escape transition checking unless guard
- [ ] 4.2 Implement unless expansion in codegen
- [ ] 4.3 Add test: unless handler interrupts main body when guard becomes enabled
- [ ] 4.4 Add test: unless handler runs exactly once
- [ ] 4.5 Add test: nested unless

## Phase 5: Acceptance Cycle Detection

- [ ] 5.1 Track accepting states in Büchi automaton (from ltl2ba)
- [ ] 5.2 Implement second DFS: for each accepting state, DFS that only follows accepting-state paths
- [ ] 5.3 Detect cycle when second DFS finds a state already on its stack
- [ ] 5.4 Report acceptance cycle violation with trail
- [ ] 5.5 Add test: liveness property `[]<>p` detects violation when p stops holding
- [ ] 5.6 Add test: liveness property `<>[]p` detects violation when p never stabilizes
- [ ] 5.7 Add test: liveness property holds when it should

## Phase 6: Non-Progress Cycle Detection

- [ ] 6.1 Add `_progress` bit to state vector
- [ ] 6.2 Track progress labels during codegen
- [ ] 6.3 Set `_progress` bit when transition visits a progress label
- [ ] 6.4 Implement non-progress cycle detection: second DFS for cycles with no progress
- [ ] 6.5 Add test: model with progress labels and non-progress cycle
- [ ] 6.6 Add test: model with progress labels and no non-progress cycle
