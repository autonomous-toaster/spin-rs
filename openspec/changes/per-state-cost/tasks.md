# Tasks

## Phase 1: The Instrument

- [ ] 1.1 Commit the throughput fixture: the N=60 chain generator that yields 481 states, 599 transitions and 422 blob entries, replacing the uncommitted `/tmp/c60.pml`
- [ ] 1.2 Fix or delete `bench_split_parse_vs_lua`: it is an unconditional `#[test]` that reads `/tmp/c60.pml` and panics on a clean checkout
- [ ] 1.3 Time the expansion by section at 422 entries: `table_from_blob` alone, `_spin_get_transitions` alone, one `_spin_clone`, one effect, one `serialize_table`
- [ ] 1.4 Report states/s, us/state, us/entry and us/transition with the startup share, median of three runs
- [ ] 1.5 Record the baseline table, verdicts and state counts included, in this change

## Phase 2: The Transition Table

- [ ] 2.1 Build each proctype's step table - guards, effects, metadata - once per model instead of once per state visit
- [ ] 2.2 Bind the state as an argument to the effect rather than capturing it in a fresh closure
- [ ] 2.3 Do not pay for a blocked process: no guard closure, effect closure or descriptor table for a step that cannot fire
- [ ] 2.4 Measure 2.1-2.3 and sweep the corpus: no verdict and no chain count may change

## Phase 3: The State Representation

- [ ] 3.1 Stop reserializing the whole state per enabled transition: carry the successor as a handle and serialize at the store boundary
- [ ] 3.2 Stop deep-copying the whole table per enabled transition: copy the write set, not the state
- [ ] 3.3 Keep the store comparing full states, never a hash alone
- [ ] 3.4 Keep the projection claim-relative and applied before hashing and before storing, as `veriplan-perf-parity` T2.3-T2.5 established
- [ ] 3.5 Measure 3.1-3.4 and sweep the corpus: no verdict and no chain count may change

## Phase 4: The Gate

- [ ] 4.1 Assert the floor in a release build: at least 5700 states/s on the committed throughput fixture, median of three runs
- [ ] 4.2 Assert the per-entry cost does not grow with transitions per state: two fixtures at about 1.2 and 5 transitions per state, entries held equal
- [ ] 4.3 Report the achieved figure with the fixture size and the transition density, and state explicitly that it is not parity with pan

## Phase 5: Verdicts

- [ ] 5.1 Sweep the corpus: no violation, unverified entry or clean verdict may change
- [ ] 5.2 Assert the chain probes are unchanged: 49/65/81/97 states at N=6/8/10/12, verdicts matching pan
