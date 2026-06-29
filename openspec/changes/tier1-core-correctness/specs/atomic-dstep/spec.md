# Atomic / D-Step

## Task Reference

| Task ID | Description |
|---------|-------------|
| T2.1 | Design state machine expansion for atomic blocks |
| T2.2 | Implement atomic expansion in codegen |
| T2.3 | Implement d_step expansion in codegen |
| T2.4 | Test: atomic block with failing guard retries |
| T2.5 | Test: d_step block produces no intermediate states |
| T2.6 | Test: nested atomic inside do-loop |

## ADDED Requirements

### Requirement: Atomic Expansion

T2.1 SHALL complete BEFORE T2.2 SHALL run. An atomic block with N inner statements SHALL expand to N+1 states: entry state (checks first guard), N-1 intermediate states (each checks next guard), and a reset state (atomic completed). On any inner guard failure, the state SHALL reset to the entry state.

#### Scenario: ATOMIC-1: Atomic retry on guard failure

GIVEN an atomic block `atomic { a > 0; b = 1 }` where `a` is initially 0
WHEN T2.2 runs
THEN the atomic block SHALL retry from the start until `a > 0` becomes true.

### Requirement: D-Step Expansion

T2.1 SHALL complete BEFORE T2.3 SHALL run. A d_step block SHALL expand identically to atomic, except intermediate states SHALL NOT be stored in the visited set (they are transient).

#### Scenario: DSTEP-1: D-step transient states

GIVEN a d_step block `d_step { x = 1; y = 2 }`
WHEN T2.3 runs
THEN the visited set SHALL contain only the state before and after the d_step, not the intermediate state where x=1 and y=0.

### Requirement: Intermediate State Storage

T2.5 SHALL complete AFTER T2.4 SHALL run. Atomic intermediate states SHALL be stored in the visited set. D-step intermediate states SHALL NOT be stored.

#### Scenario: Intermediate State Storage scenario

GIVEN the requirement is implemented
WHEN the system runs
THEN the behavior SHALL be correct
