# Trail Replay
## Task Reference

| Task ID | Description |
|---------|-------------|
| T2.1 | Extend TrailReplayer to dump state at each step |
| T2.2 | Add --inspect flag for state dumps |
| T2.3 | Support Spin-compatible trail format for reading |
| T2.4 | Support Spin-compatible trail format for writing |
| T2.5 | Add -t CLI option |
| T2.6 | Add -k CLI option |
| T2.7 | Test: trail replay with state inspection |


## ADDED Requirements

### Requirement: State Dump During Replay
T2.1 SHALL complete BEFORE T2.2 SHALL run. The TrailReplayer SHALL dump the full state vector at each step during replay. T2.2 SHALL add an `--inspect` flag that enables state dumps.

#### Scenario: TRAIL-1: Trail replay with state inspection
GIVEN a saved error trail
WHEN T2.7 runs with `-t -k error.trail --inspect`
THEN each step SHALL display the state vector values.

### Requirement: Spin-Compatible Format
T2.3 SHALL complete BEFORE T2.5 SHALL run. The trail reader SHALL parse Spin's `.trail` file format. T2.4 SHALL write trails in Spin-compatible format. T2.5 SHALL add `-t` to replay a trail. T2.6 SHALL add `-k` to specify the trail file path.

#### Scenario: Spin-Compatible Format scenario

GIVEN the requirement is implemented
WHEN the system runs
THEN the behavior SHALL be correct
