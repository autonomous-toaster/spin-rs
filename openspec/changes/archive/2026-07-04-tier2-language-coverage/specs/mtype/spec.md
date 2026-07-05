# Mtype
## Task Reference

| Task ID | Description |
|---------|-------------|
| T1.1 | Add mtype declaration parsing |
| T1.2 | Assign sequential integer IDs to mtype names |
| T1.3 | Store mtype name-to-value mapping in AST |
| T1.4 | Emit mtype mapping table in generated Lua |
| T1.5 | Support mtype in variable declarations |
| T1.6 | Support mtype comparison |
| T1.7 | Test: mtype declaration and comparison |
| T1.8 | Test: mtype in channel send/receive |


## ADDED Requirements

### Requirement: Mtype Declaration
T1.1 SHALL complete BEFORE T1.2 SHALL run. `mtype = { red, green, blue }` SHALL be parsed as a top-level declaration. T1.2 SHALL assign IDs 0, 1, 2 to red, green, blue.

#### Scenario: MTYPE-1: Mtype declaration and comparison
GIVEN `mtype = { ready, busy }; mtype state = ready`
WHEN T1.7 runs
THEN `state == ready` SHALL evaluate to true and `state == busy` SHALL evaluate to false.

### Requirement: Mtype Mapping
T1.3 SHALL complete BEFORE T1.4 SHALL run. The AST SHALL store a mapping from mtype name to integer value. T1.4 SHALL emit this mapping as a Lua table for printm support.

#### Scenario: MTYPE-1: Mtype declaration and comparison
GIVEN `mtype = { ready, busy }; mtype state = ready`
WHEN T1.7 runs
THEN `state == ready` SHALL evaluate to true and `state == busy` SHALL evaluate to false.

### Requirement: Mtype Usage
T1.5 SHALL complete BEFORE T1.7 SHALL run. Variables of type `mtype` SHALL store integer values. T1.6 SHALL support comparison `x == red` by comparing the variable's integer value against the mtype's assigned ID.

#### Scenario: MTYPE-1: Mtype declaration and comparison
GIVEN `mtype = { ready, busy }; mtype state = ready`
WHEN T1.7 runs
THEN `state == ready` SHALL evaluate to true and `state == busy` SHALL evaluate to false.
