# Channel Initialization
## Task Reference

| Task ID | Description |
|---------|-------------|
| T5.1 | Add parsing for channel init syntax |
| T5.2 | Store field types in channel metadata |
| T5.3 | Emit channel creation with typed slots |
| T5.4 | Validate send/receive field counts |
| T5.5-T5.6 | Tests |


## ADDED Requirements

### Requirement: Channel Init Parsing
T5.1 SHALL complete BEFORE T5.2 SHALL run. `chan q = [5] of { byte, int }` SHALL be parsed as a channel declaration with capacity 5 and two fields (byte, int).

#### Scenario: CHANINIT-1: Typed channel send/receive
GIVEN `chan q = [5] of { byte, int }`
WHEN T5.5 runs with `q!1,2` and `q?x,y`
THEN x SHALL equal 1 and y SHALL equal 2.

### Requirement: Field Type Validation
T5.2 SHALL complete BEFORE T5.4 SHALL run. The channel metadata SHALL store the field types. T5.4 SHALL validate that send and receive operations use the correct number of fields.

#### Scenario: CHANINIT-2: Field count mismatch
GIVEN `chan q = [5] of { byte, int }`
WHEN T5.6 runs with `q!1` (missing int field)
THEN a parse or runtime error SHALL be reported.
