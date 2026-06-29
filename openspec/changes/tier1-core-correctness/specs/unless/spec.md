# Unless

## Task Reference

| Task ID | Description |
|---------|-------------|
| T4.1 | Design unless expansion |
| T4.2 | Implement unless expansion in codegen |
| T4.3 | Test: unless handler interrupts main body |
| T4.4 | Test: unless handler runs exactly once |
| T4.5 | Test: nested unless |

## ADDED Requirements

### Requirement: Unless Expansion Design

For each step in the main body of an escape construct, the codegen SHALL emit an escape transition that checks the escape guard. When the escape guard is enabled, control transfers to the escape handler body. T4.1 SHALL complete BEFORE T4.2 SHALL run.

#### Scenario: UNLESS-1: Unless interrupts main body

GIVEN `{ body } unless { handler }` where body is a long-running sequence
WHEN T4.2 runs
THEN when the unless guard becomes enabled at any step, the handler SHALL execute instead of continuing the body.

#### Scenario: UNLESS-2: Unless runs once

GIVEN `{ do :: true -> skip od } unless { x = 1 }`
WHEN T4.2 runs
THEN the handler SHALL execute exactly once and then the process SHALL terminate.

### Requirement: Unless Handler Execution

The escape handler runs exactly once when its guard becomes enabled. After the handler completes, the process terminates (or continues per Promela semantics). T4.2 SHALL complete BEFORE T4.4 SHALL run.

#### Scenario: UNLESS-1: Unless interrupts main body

GIVEN `{ body } unless { handler }` where body is a long-running sequence
WHEN T4.2 runs
THEN when the unless guard becomes enabled at any step, the handler SHALL execute instead of continuing the body.

#### Scenario: UNLESS-2: Unless runs once

GIVEN `{ do :: true -> skip od } unless { x = 1 }`
WHEN T4.2 runs
THEN the handler SHALL execute exactly once and then the process SHALL terminate.
