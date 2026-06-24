## Context

spin-rs aims to replace the external Spin binary as veriplan's model checking backend. The benchmark suite provides empirical validation of correctness equivalence.

Current state after recent fixes:

- **Progress**: `plan_5tasks` 1→10 states (28%), `plan_20tasks` 1→233 states (12%)
- **Regression**: `single_loop` 102→2 states due to over-applied boolean checks
- **Blocking**: Channel syntax `chan = [N] of { type }` doesn't parse (0 declarations)
- **Blocking**: LTL verification not integrated into benchmark

## Goals

**Must Have:**

- Parse `chan name = [N] of { type };` syntax correctly
- Wire `verify_ltl()` into benchmark for LTL models
- Fix boolean `~= 0` checks to apply ONLY in guard contexts
- `deadlock_circular` detects 1 error (deadlock)
- `ltl_violation` detects 1 error (LTL violation)
- `single_loop` returns to 102+ states

**Nice to Have:**

- Document all channel syntax variants supported
- Add regression tests for boolean scoping

**Out of Scope:**

- Channel array enhancements (separate change)
- Multi-field message support
- Performance optimizations

## Decisions

### D1: Channel Syntax Parsing Strategy

**Chosen**: Add separate parser rule for `chan name = [N] of { type }`

**Rationale**:

- Existing `chan_array_decl` handles `chan name[N]`
- Different semantics: single channel with capacity vs array of channels
- Keeps parser modular and maintainable

**Alternative Considered**: Merge both syntaxes into one rule
**Rejected**: Would complicate parser, harder to maintain

### D2: Boolean Check Context Tracking

**Chosen**: Add `in_guard_context` flag to codegen expression methods

**Rationale**:

- Need to distinguish guard expressions from assignment expressions
- Minimal API change: add boolean parameter to existing methods
- Preserves existing code structure

**Alternative Considered**: Separate methods for guard vs non-guard expressions
**Rejected**: Would duplicate code, harder to maintain consistency

### D3: LTL Integration Approach

**Chosen**: Detect LTL formulas in model, call `verify_ltl()` per formula

**Rationale**:

- `verify_ltl()` already exists and works
- Minimal integration effort
- Matches Spin's error counting (1 error per violated property)

**Alternative Considered**: Unified `verify()` that handles both safety and LTL
**Rejected**: Would require larger refactoring, current approach is simpler

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  Phase 1: Channel Syntax Parser                             │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Parser: Add chan_decl rule                                 │
│    chan name = [N] of { type }                              │
│           │                                                 │
│           ▼                                                 │
│    TopLevel::ChanDecl { name, capacity, msg_type }         │
│           │                                                 │
│           ▼                                                 │
│  Codegen: Emit channel state in _spin_init_state           │
│    state.ch_name = nil  (or capacity tracking)             │
│           │                                                 │
│           ▼                                                 │
│  Runtime: Wire ChanDecl in from_model()                    │
│    Register channels with correct capacity                 │
│                                                             │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│  Phase 2: LTL Integration                                   │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Benchmark: Detect LTL in model                            │
│    if model.declarations.has(Ltl)                          │
│           │                                                 │
│           ▼                                                 │
│    Call verify_ltl(source, formula, name)                  │
│           │                                                 │
│           ▼                                                 │
│    Compare errors with Spin                                │
│                                                             │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│  Phase 3: Boolean Scoping                                   │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Codegen: Track guard context                              │
│                                                             │
│  expr_to_lua(expr, in_guard: bool)                         │
│    │                                                        │
│    ├─ in_guard=true:  add ~= 0 for bools                   │
│    │   Expression::Ident(name) => "s.x ~= 0"               │
│    │                                                        │
│    └─ in_guard=false: no ~= 0                              │
│        Expression::Ident(name) => "s.x"                    │
│                                                             │
│  Usage:                                                     │
│    emit_guards() → expr_to_lua(e, true)   ✓               │
│    emit_assignment() → expr_to_lua(e, false) ✓            │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## Risks & Trade-offs

### Risk: Channel Type Parsing Complexity

**Mitigation**: Start with basic types (byte, int, bool), defer complex types

### Risk: Boolean Scoping Misses Edge Cases

**Mitigation**: Comprehensive test suite covering all expression contexts

### Risk: LTL Performance Overhead

**Mitigation**: LTL verification is already implemented, just integrating

## Testing Strategy

### Unit Tests

- Parser: `chan ch = [0] of { byte };` → `ChanDecl`
- Codegen: Boolean checks in guards vs assignments
- Runtime: Channel registration with capacity

### Integration Tests

- `deadlock_circular`: 1 error (deadlock detected)
- `ltl_violation`: 1 error (LTL violation detected)
- `single_loop`: 102+ states (regression fixed)

### Benchmark Validation

- Remove `token_ring` skip (once channels work)
- Compare all models against Spin
- Target: 90%+ state count match on non-channel models
