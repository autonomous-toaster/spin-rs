# Channel Operations

## Task Reference

| Task ID | Description |
|---------|-------------|
| T3.1 | Add `chan_send_sorted` to runtime |
| T3.2 | Add `chan_recv_random` to runtime |
| T3.3 | Add `chan_poll` to runtime |
| T3.4 | Add `chan_recv_eval` to runtime |
| T3.5 | Expose new channel ops as Lua FFI functions |
| T3.6 | Update codegen to emit `!!` as sorted send |
| T3.7 | Update codegen to emit `??` as random receive |
| T3.8 | Update codegen to emit `?<expr>` as poll receive |
| T3.9 | Update codegen to emit `eval(expr)` in receive |
| T3.10 | Test: sorted send maintains order |
| T3.11 | Test: random receive picks different messages |
| T3.12 | Test: poll receive does not consume message |
| T3.13 | Test: eval receive matches specific value |

## ADDED Requirements

### Requirement: Sorted Send

T3.1 SHALL complete BEFORE T3.6 SHALL run. `chan_send_sorted` SHALL insert the message into the channel buffer at the position that maintains sorted order (ascending by message value). T3.10 SHALL complete AFTER T3.6.

#### Scenario: SORTED-1: Sorted send maintains order

GIVEN a channel with messages [3, 1, 2] sent via `!!`
WHEN T3.6 runs
THEN the channel buffer SHALL contain [1, 2, 3] in ascending order.

### Requirement: Random Receive

T3.2 SHALL complete BEFORE T3.7 SHALL run. `chan_recv_random` SHALL non-deterministically select any message in the channel buffer and remove it. T3.11 SHALL complete AFTER T3.7.

#### Scenario: RANDOM-1: Random receive picks different messages

GIVEN a channel with messages [1, 2, 3]
WHEN T3.7 runs across multiple verification runs
THEN different messages SHALL be selected non-deterministically.

#### Scenario: EVAL-1: Eval receive matches value

GIVEN a channel with message [5]
WHEN T3.9 runs with `ch ? eval(5)`
THEN the message SHALL be consumed and the receive SHALL succeed.

### Requirement: Poll Receive

T3.3 SHALL complete BEFORE T3.8 SHALL run. `chan_poll` SHALL check whether the first message in the channel matches a given expression WITHOUT consuming it. T3.12 SHALL complete AFTER T3.8.

#### Scenario: RANDOM-1: Random receive picks different messages

GIVEN a channel with messages [1, 2, 3]
WHEN T3.7 runs across multiple verification runs
THEN different messages SHALL be selected non-deterministically.

#### Scenario: POLL-1: Poll does not consume

GIVEN a channel with message [5]
WHEN T3.8 runs with `ch ?[5]`
THEN the channel SHALL still contain [5] after the poll.

#### Scenario: EVAL-1: Eval receive matches value

GIVEN a channel with message [5]
WHEN T3.9 runs with `ch ? eval(5)`
THEN the message SHALL be consumed and the receive SHALL succeed.

### Requirement: Eval Receive

T3.4 SHALL complete. T3.9 SHALL run after T3.4 completes. `chan_recv_eval` SHALL receive a message only when the first message in the channel matches a given value. T3.13 SHALL complete after T3.9.

#### Scenario: RANDOM-1: Random receive picks different messages

GIVEN a channel with messages [1, 2, 3]
WHEN T3.7 runs across multiple verification runs
THEN different messages SHALL be selected non-deterministically.

#### Scenario: EVAL-1: Eval receive matches value

GIVEN a channel with message [5]
WHEN T3.9 runs with `ch ? eval(5)`
THEN the message SHALL be consumed and the receive SHALL succeed.
