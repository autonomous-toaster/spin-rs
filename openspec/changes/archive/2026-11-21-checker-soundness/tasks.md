# Tasks

## Phase 1: Parse Integrity

- [x] 1.1 Accept an optional trailing semicolon after `fi` and `od` in `if_stmt` / `do_stmt`
- [x] 1.2 Reject trailing unparsed input in `parse()` instead of truncating the model
- [x] 1.3 Classify keyword-led statements as statements in `guard_body`
- [x] 1.4 Regression test: `if/fi;` nested in `do/od` explores more than one state

## Phase 2: Verdict Integrity

- [x] 2.1 Report an error when a model has at least one proctype and zero transitions
- [x] 2.2 Surface Lua evaluation failures instead of mapping them to empty transitions
- [x] 2.3 Deduplicate violations per property

## Phase 3: Property Evaluation

- [x] 3.1 Make an unresolved atomic proposition an error naming the atom, never `false`
- [x] 3.2 Reject non-identifier atoms explicitly, or evaluate them as expressions
- [x] 3.3 Evaluate atomic propositions in the successor state in `sync_transitions`

## Phase 4: Differential Conformance

- [x] 4.1 Add m1/m2/m3 and the veriplan `if/fi` shape to the bench corpus
- [x] 4.2 Compare per-property error counts and explored states against real spin
