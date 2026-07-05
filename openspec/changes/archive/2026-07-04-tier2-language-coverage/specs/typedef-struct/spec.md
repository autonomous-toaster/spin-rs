# Typedef / Struct
## Task Reference

| Task ID | Description |
|---------|-------------|
| T2.1 | Add typedef parsing |
| T2.2 | Add struct variable declaration parsing |
| T2.3 | Add struct field access parsing |
| T2.4 | Add struct assignment parsing |
| T2.5 | Emit struct fields as nested Lua tables |
| T2.6 | Emit struct field access as table field access |
| T2.7 | Emit struct assignment as table copy |
| T2.8 | Test: struct declaration and field access |
| T2.9 | Test: struct assignment |
| T2.10 | Test: struct array |


## ADDED Requirements

### Requirement: Typedef Parsing
T2.1 SHALL complete BEFORE T2.2 SHALL run. `typedef MyStruct { byte a; int b }` SHALL be parsed and the field layout SHALL be stored.

#### Scenario: Typedef Parsing scenario

GIVEN the requirement is implemented
WHEN the system runs
THEN the behavior SHALL be correct

### Requirement: Struct Field Access
T2.3 SHALL complete BEFORE T2.6 SHALL run. `s.a` SHALL access the `a` field of struct variable `s`. T2.6 SHALL emit `state.s.a` in Lua.

#### Scenario: STRUCT-1: Struct field access
GIVEN `typedef Msg { byte src; byte dst }; Msg m`
WHEN T2.8 runs
THEN `m.src = 5` SHALL set the src field and `m.dst = 3` SHALL set the dst field independently.

#### Scenario: STRUCT-2: Struct assignment
GIVEN two struct variables `a` and `b` of the same type
WHEN T2.9 runs with `a = b`
THEN all fields of `a` SHALL equal the corresponding fields of `b`.

### Requirement: Struct Assignment
T2.4 SHALL complete BEFORE T2.7 SHALL run. `s = t` SHALL copy all fields from `t` to `s`. T2.7 SHALL emit a field-by-field copy in Lua.

#### Scenario: STRUCT-1: Struct field access
GIVEN `typedef Msg { byte src; byte dst }; Msg m`
WHEN T2.8 runs
THEN `m.src = 5` SHALL set the src field and `m.dst = 3` SHALL set the dst field independently.

#### Scenario: STRUCT-2: Struct assignment
GIVEN two struct variables `a` and `b` of the same type
WHEN T2.9 runs with `a = b`
THEN all fields of `a` SHALL equal the corresponding fields of `b`.
