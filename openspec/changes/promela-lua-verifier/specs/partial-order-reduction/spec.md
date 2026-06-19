## Task Reference

| Task ID | Description |
|---------|-------------|
| T6.1 | Implement persistent-set partial order reduction |
| T6.2 | Verify POR correctness against exhaustive search |

## ADDED Requirements

### Requirement: Persistent-set POR

T6.1 SHALL ALWAYS compute a subset of enabled transitions at each state using persistent-set (ample-set) partial order reduction. T6.1 SHALL be disabled by default; enabled via `-DPOR` flag.

#### Scenario: Independent transitions

- **WHEN** T6.1 encounters two transitions from different proctypes accessing disjoint variables
- **THEN** T6.1 SHALL explore only one ordering instead of both

#### Scenario: Dependent transitions

- **WHEN** T6.1 encounters two transitions accessing the same variable
- **THEN** T6.1 SHALL explore both orderings (cannot reduce)

### Requirement: POR correctness verification

T6.2 SHALL ALWAYS verify soundness by comparing reduced and full state spaces. T6.2 SHALL complete AFTER T6.1 SHALL implement POR.

#### Scenario: POR validation

- **WHEN** T6.2 runs a small model with and without POR enabled
- **THEN** T6.2 SHALL verify that both runs find the same set of property violations
