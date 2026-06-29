## ADDED Requirements

### Task Reference

| Task ID | Description |
|---------|-------------|
| T7.1 | Fix `LuaChannel::send` to reject sends on capacity-0 channels |
| T7.2 | Document rendezvous limitation in design |
| T7.3 | Verify deadlock_circular still detects 1 error after fix |

### Requirement: Rendezvous send blocks

T7.1 SHALL ALWAYS return `false` from `LuaChannel::send` when `capacity == 0`. A capacity-0 (rendezvous) channel SHALL NOT accept a send unless a matching recv is simultaneously available. Since the flat transition model cannot pair send and recv atomically, the conservative fix SHALL make capacity-0 channels never sendable — forcing both sender and receiver to be ready simultaneously (which at present only happens in deadlock scenarios).

#### Scenario: Rendezvous send rejected

- **WHEN** T7.1 calls `send` on a channel with `capacity == 0`
- **THEN** `send` SHALL return `false`

#### Scenario: Buffered send unchanged

- **WHEN** T7.1 calls `send` on a channel with `capacity > 0`
- **THEN** `send` SHALL behave as before (accept if space available)

### Requirement: Rendezvous limitation documented

T7.2 SHALL ALWAYS document in the design that capacity-0 channels never become sendable, making rendezvous communication impossible until a proper send/recv pairing mechanism is implemented. This is a conservative fix that prevents incorrect exploration (non-blocking sends on rendezvous channels) at the cost of blocking models that depend on rendezvous for progress.

### Requirement: deadlock_circular unchanged

T7.3 SHALL ALWAYS verify that `deadlock_circular` model still detects exactly 1 error after the rendezvous fix. The model uses capacity-0 channels, but the deadlock is detected via recv (both processes are blocked on recv from empty channels) — not via send.

#### Scenario: deadlock still found

- **WHEN** T7.3 runs `verify(DEADLOCK_CIRCULAR)` after rendezvous fix
- **THEN** `result.errors` SHALL equal 1
