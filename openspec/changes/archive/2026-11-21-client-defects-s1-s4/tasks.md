# Tasks

## Phase 1: S1 Liveness Refutation

- [x] 1.1 Stutter the never claim at terminal model states
- [x] 1.2 Regression test: violated <> over a terminating run is refuted

## Phase 2: S4 Unverified

- [x] 2.1 Complement [] (a -> <> b)
- [x] 2.2 Add CheckResult::unverified and stop counting undecidable properties as errors
- [x] 2.3 Record max_states truncation in unverified
- [x] 2.4 CLI reports an inconclusive run instead of success
- [x] 2.5 Regression test: a satisfied conditional is not an error

## Phase 3: S2 and S3 Documentation and Release

- [x] 3.1 README documents the three outcomes, Err conditions, transition rule, deadline, root causes
- [x] 3.2 Remove the unsupported U/V claim from the feature list
- [x] 3.3 Bump the manifest to 0.2.0 and release v0.2.0
