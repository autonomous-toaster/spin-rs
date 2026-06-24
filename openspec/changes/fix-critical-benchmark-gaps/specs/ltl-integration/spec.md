# LTL Integration - Remaining Work

## Overview

This spec documents the remaining work needed to complete LTL verification in spin-rs. Phase 2 of the `fix-critical-benchmark-gaps` change was partially implemented but blocked by a fundamental mismatch between our LTL formula representation and the ltl2ba library's expectations.

## Current State

### ✅ Completed

- LTL formulas are parsed from Promela source and stored in `LuaModel`
- LTL detection integrated into checker (prevents benchmark hang)
- Büchi automaton conversion implemented for `[]p`, `<>p`, `Xp` formulas
- Atomic proposition evaluation extracts variable values from state blob
- Nested DFS infrastructure in place for cycle detection

### ❌ Blocking Issue

**Root Cause**: The ltl2ba library expects atomic propositions to be simple identifiers (e.g., `p`, `q`, `flag`), but our LTL formulas contain expressions (e.g., `x == 0`, `t2_1`).

**Example**:

```promela
ltl p0 { [](x == 0) }
```

- Our parser creates: `LtlFormula::Always(LtlFormula::Atom("x == 0"))`
- ltl2ba parser expects: `LtlFormula::Always(LtlFormula::Atom("p"))` where `p` is a simple name
- When we try to parse `"x == 0"` with ltl2ba, it fails: `Parse error at position 2: Expected ')'`

**Impact**:

- `ltl_violation` benchmark shows 0 errors instead of expected 1
- `plan_5tasks_3ltls` state counts differ (spin-rs: 32, Spin: 36)
- LTL verification runs but doesn't actually verify the formulas

## Solution Options

### Option 1: Pre-process LTL Formulas (RECOMMENDED)

**Approach**: Before passing formulas to ltl2ba, replace complex expressions with simple placeholder names, then map those names back during atomic prop evaluation.

**Steps**:

1. Parse LTL formula to extract all atomic expressions (`x == 0`, `t2_1`, etc.)
2. Create a mapping: `{"x == 0" → "atom_0", "t2_1" → "atom_1"}`
3. Rewrite formula: `[](x == 0)` → `[](atom_0)`
4. Pass rewritten formula to ltl2ba for Büchi conversion
5. During nested DFS, evaluate atomic props using the mapping

**Pros**:

- Minimal changes to ltl2ba library
- Preserves existing infrastructure
- Can handle any expression our parser supports

**Cons**:

- Need to maintain expression-to-name mapping throughout verification
- Slight overhead in atomic prop evaluation

**Implementation Estimate**: 4-6 hours

### Option 2: Extend ltl2ba Parser

**Approach**: Modify ltl2ba parser to accept expressions like `x == 0` as atomic propositions.

**Steps**:

1. Update `parse_atom_or_paren` in `src/property/ltl2ba/parser.rs`
2. Allow expressions (not just identifiers) as atoms
3. Update Büchi conversion to handle expression strings

**Pros**:

- More natural representation
- No need for pre-processing

**Cons**:

- Requires deeper changes to ltl2ba
- Expression evaluation still needs to be implemented in `evaluate_atomic_props`
- May conflict with ltl2ba's internal assumptions

**Implementation Estimate**: 6-8 hours

### Option 3: Use Spin's Never Claims

**Approach**: Instead of LTL → Büchi conversion, generate never claims like Spin does.

**Steps**:

1. Parse LTL formula
2. Generate never claim (Promela code)
3. Compose never claim with model
4. Verify using standard safety checking

**Pros**:

- Matches Spin's approach exactly
- No Büchi automaton needed

**Cons**:

- Requires never claim generation logic
- More complex than Option 1
- Duplicates some ltl2ba functionality

**Implementation Estimate**: 8-12 hours

## Recommended Implementation Plan

### Phase 2.1: Pre-processing Layer (4-6 hours)

1. **Create formula pre-processor** (`src/property/ltl_preprocess.rs`):
   - Extract atomic expressions from LTL formula
   - Generate unique names for each expression
   - Rewrite formula with placeholder names
   - Maintain bidirectional mapping

2. **Update `evaluate_atomic_props`**:
   - Accept mapping as parameter
   - Evaluate expressions against state blob
   - Return truth values for placeholder names

3. **Update `PropertyChecker::check_liveness`**:
   - Call pre-processor before Büchi conversion
   - Pass mapping to nested DFS
   - Use mapping during atomic prop evaluation

### Phase 2.2: Testing & Validation (2-3 hours)

1. **Unit tests**:
   - Pre-processor correctly extracts atoms
   - Rewritten formulas parse with ltl2ba
   - Atomic prop evaluation works with mapping

2. **Integration tests**:
   - `ltl_violation` detects 1 error
   - `plan_5tasks_3ltls` state counts match Spin (within tolerance)

3. **Benchmark validation**:
   - All LTL models pass correctness checks
   - No new regressions introduced

### Phase 2.3: Documentation (1 hour)

1. Update `openspec/changes/fix-critical-benchmark-gaps/specs/ltl-benchmark/spec.md`
2. Document pre-processing approach in code comments
3. Add examples to property module docs

## Acceptance Criteria

- [ ] `ltl_violation` benchmark detects exactly 1 error
- [ ] `plan_5tasks_3ltls` state counts within 10% of Spin
- [ ] All existing tests still pass
- [ ] No new benchmark regressions
- [ ] Code reviewed and merged

## Dependencies

- None (standalone improvement)
- Builds on existing ltl2ba infrastructure
- Compatible with current `LuaModel` and `PropertyChecker` APIs

## Risks

- **Low**: Pre-processing is a thin layer, minimal changes to core logic
- **Medium**: Expression evaluation must handle all expression types from parser
- **Low**: Performance overhead should be negligible (one-time per formula)

## Notes

- Current implementation already has 80% of infrastructure in place
- Main gap is the atom name mismatch
- Pre-processing is the most surgical fix with least risk
- Can be implemented incrementally (one formula type at a time)
