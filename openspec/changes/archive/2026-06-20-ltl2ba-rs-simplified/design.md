## Context

spin-rs v2 requires LTL verification via Büchi automata. The omega-automata crate provides LTL → Büchi conversion but keeps the NBW structure private, blocking integration. Full ltl2ba implementation requires 4-6 weeks of complex automata-theory code (~2000 LOC).

**Current state**: `BuchiAutomaton::from_ltl()` returns a trivial 1-state automaton (stub).

**Stakeholders**:

- spin-rs users needing LTL verification
- VeriPlan (embeds spin-rs for model checking)
- Future maintainers (need clear migration path to full LTL)

## Goals / Non-Goals

**Goals:**

- Support ~60-70% of real-world LTL properties ([]p, <>p, Xp, boolean combinations)
- Clear error messages for unsupported operators (U, V, nested temporal)
- Clean API that allows drop-in replacement with full ltl2ba later
- Minimal LOC (~500-800) for simplified implementation
- Integration with spin-rs property module without breaking changes

**Non-Goals:**

- Full LTL support (until, release, nested temporal operators) — deferred to v2.1
- Automata minimization — simple construction is sufficient
- Performance optimization — correctness first, optimize later
- Standalone crate publication — internal to spin-rs initially

## Decisions

### D1: Simplified Operator Set

**Decision**: Support only `[]p`, `<>p`, `Xp`, `!p`, `p && q`, `p || q` where `p`, `q` are atomic propositions.

**Rationale**:

- These cover ~60-70% of real-world properties (based on Spin model surveys)
- Each has simple, well-known automaton structure (1-3 states)
- Can be implemented independently (no complex tableau needed)
- Clear error messages for unsupported operators

**Alternatives considered**:

- Full LTL support: Too complex for v2.0 timeline (4-6 weeks)
- No LTL support: Blocks v2 correctness guarantees
- omega-automata with public accessors: Uncertain timeline, external dependency

### D2: Pattern-Based Construction

**Decision**: Implement pattern matching on normalized formulas rather than general tableau.

**Rationale**:

- Each supported pattern has known optimal automaton
- Simpler code (~500 LOC vs ~2000 LOC for full tableau)
- Easier to test (each pattern has expected automaton structure)
- Can add patterns incrementally

**Automaton patterns**:

```
[]p:     s0 --p--> s0 (accepting), s0 --!p--> s1 (rejecting sink)
<>p:     s0 --p--> s0 (accepting), s0 --!p--> s0 (loop), s0 --p--> s1 (accepting)
Xp:      s0 --any--> s1, s1 --p--> s1 (accepting), s1 --!p--> s2 (rejecting)
```

### D3: Error Handling

**Decision**: Return `Result<BuchiAutomaton, LtlError>` with specific error variants for unsupported operators.

**Rationale**:

- Users get clear feedback on what's supported
- Easier to debug than panics
- Allows graceful degradation (try simplified, fall back to Spin)

**Error variants**:

```rust
enum LtlError {
    UnsupportedOperator { op: String, suggestion: Option<String> },
    NestedTemporal { formula: String },
    ParseError { message: String },
}
```

### D4: API Compatibility

**Decision**: Match the API that full ltl2ba would provide, enabling drop-in replacement.

**Rationale**:

- No API changes when upgrading to full implementation
- Users can test with simplified, deploy with full
- Easier to maintain two implementations with same interface

**Key API**:

```rust
pub fn parse_ltl(input: &str) -> Result<LtlFormula, LtlError>;
pub fn to_buchi(formula: &LtlFormula) -> Result<BuchiAutomaton, LtlError>;
```

## Risks / Trade-offs

| Risk | Impact | Mitigation |
|------|--------|------------|
| Users need unsupported operators | Can't verify their properties | Clear documentation, error messages suggest Spin workaround |
| Simplified automata incorrect | Missed violations | Extensive testing against known formulas, compare with Spin |
| Migration to full ltl2ba delayed | Stuck with limited LTL | Design API for drop-in replacement, track full implementation as separate work |
| Performance worse than omega-automata | Slower verification | Benchmark early, optimize hot paths (state hashing, transition matching) |
| Code duplication (simplified + full later) | Maintenance burden | Shared API, common data structures (`BuchiAutomaton`, `LtlFormula`) |

## Migration Plan

### Phase 1: Simplified Implementation (2-3 weeks)

1. Create `ltl2ba-rs-simplified` module structure
2. Implement parser for LTL strings
3. Implement pattern-based Büchi construction
4. Add error handling for unsupported operators
5. Integrate with `BuchiAutomaton::from_ltl()`
6. Test against standard formulas

### Phase 2: Integration (1 week)

1. Update `src/property/buchi.rs` to use simplified implementation
2. Update nested DFS to work with constructed automata
3. Add integration tests (LTL properties on Promela models)
4. Document limitations in README

### Phase 3: Full Implementation (future, 4-6 weeks)

1. Implement full tableau construction (or port ltl2ba)
2. Replace `simple.rs` with `full.rs` (same API)
3. Test against full LTL suite
4. Update documentation

**Rollback**: Revert to stub implementation if bugs found (no user-visible change except LTL stops working)

## Open Questions

1. **Should we support `p -> q` as syntactic sugar for `!p || q`?**
   - Pro: More intuitive for users
   - Con: Adds parsing complexity
   - **Tentative**: Yes, simple rewrite

2. **Should nested temporal (`[]<>p`) be rejected or approximated?**
   - Pro (reject): Clear limitations
   - Pro (approximate): Some properties might still be verified correctly
   - **Tentative**: Reject with clear error

3. **Should ltl2ba-rs-simplified be a separate crate from the start?**
   - Pro: Reusability, clearer boundaries
   - Con: More complexity initially
   - **Tentative**: Internal module first, extract later if useful

4. **What's the acceptance threshold for "correctness"?**
   - Must match Spin on all supported formulas?
   - Or "best effort" with known limitations documented?
   - **Tentative**: Must match Spin on supported subset
