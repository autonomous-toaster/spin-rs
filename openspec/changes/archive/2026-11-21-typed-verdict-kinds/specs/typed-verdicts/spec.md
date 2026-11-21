# Typed Verdicts
## Task Reference

| Task ID | Description |
|---------|-------------|
| T1.1 | Add UnverifiedKind and populate it at every unverified site |
| T1.2 | Add VerifyError with Display, Error and kind() |
| T1.3 | Return the typed payload from verify and verify_with_config |
| T1.4 | Tests for the downcastable error and the StateBudget kind |

## ADDED Requirements

### Requirement: Unverified Results Are Classifiable
T1.1 SHALL complete BEFORE T1.4 SHALL run. Every unverified result SHALL carry ALWAYS a `kind` tag drawn from a fixed set, in addition to its human-readable reason.

#### Scenario: T1.1 truncated search
GIVEN a search that stops at `max_states`
WHEN T1.1 records it
THEN `unverified` SHALL contain an entry whose kind is `StateBudget`.

### Requirement: Errors Are Classifiable
T1.2 SHALL complete BEFORE T1.3 SHALL run. An error returned by `verify` SHALL carry ALWAYS a typed payload exposing a stable `kind`, so a caller need not match message text.

#### Scenario: T1.2 evaluation failure
GIVEN a model whose generated guard raises at evaluation time
WHEN T1.2 returns the failure
THEN the error SHALL downcast to `VerifyError` with kind `eval_failed`.

### Requirement: Entry Point Unchanged
T1.3 SHALL complete BEFORE T1.4 SHALL run. `verify` SHALL keep ALWAYS the signature `&str -> anyhow::Result<CheckResult>`, so existing callers compile unchanged.

#### Scenario: T1.3 existing caller
GIVEN a caller that already maps `Err` to "unverified"
WHEN T1.3 lands
THEN that caller SHALL compile and behave the same, and MAY additionally downcast for the kind.
