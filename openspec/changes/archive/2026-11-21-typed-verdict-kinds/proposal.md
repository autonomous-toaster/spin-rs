# Typed Verdict Kinds

## Problem

A caller that maps a violation to a requirement, or an unverified result to a finding, had to
classify by matching message text. The consumer asked twice for a stable tag, because matching
prose is the same fragility it was removing from its own `pan` output handling.

Two gaps:

- `CheckResult::unverified` carried only a free-text `reason`, so "unsupported shape", "unparsable
  formula" and "stopped at the state budget" were distinguishable only by reading English.
- `verify` and `verify_with_config` returned an opaque `anyhow::Error`, so "could not explore" and
  "evaluation failed" were likewise only distinguishable by text.

`errors == 0` with a truncated search was already reported through `unverified`; this change makes
the reason machine-readable rather than adding a new mechanism.

## What changes

- `Unverified` gains `kind: UnverifiedKind` with stable tags: `unsupported_property`,
  `unparsable_formula`, `state_budget`, `depth_budget`.
- New `VerifyError` enum carried as the payload of the `Err` from `verify` and
  `verify_with_config`: `NotExplored { processes }` and `EvalFailed { detail }`, implementing
  `Display` and `std::error::Error`, with `kind()` returning a stable tag.
- `verify(&str) -> anyhow::Result<CheckResult>` keeps its shape; the payload is downcastable.

## Non-goals

- No change to the meaning of `errors` or `violations`.
- No new entry point; `verify_with_config` remains the way to set a budget.
