# spin-rs v2: Feature Parity & Correctness

## Vision

**Make spin-rs a drop-in replacement for Spin 6.5.x** — identical verification guarantees, compatible output formats, and the ability to verify any Promela model that Spin can handle, while maintaining the v1 value proposition of "no GCC dependency, embeddable in Rust."

## Success Criteria

### Functional Parity

- ✅ Verify any Promela model that Spin 6.5.x can verify (within documented subset)
- ✅ Detect all safety violations (assertions, deadlocks) that Spin detects
- ✅ Detect all liveness violations (LTL properties) that Spin detects
- ✅ Support all Spin command-line flags that affect verification results
- ✅ Produce equivalent error trails (Spin-compatible format)

### Correctness Guarantees

- ✅ **Soundness**: Never report "no errors" when a violation exists
- ✅ **Completeness**: Never miss a violation due to approximation (unless user explicitly opts into bitstate mode)
- ✅ **POR Safety**: Partial order reduction never prunes accepting cycles (C3 condition)
- ✅ **LTL Correctness**: Full Büchi automaton construction for all LTL formulas

### Performance Targets

- ✅ Within 5x of Spin's speed on equivalent models (acceptable for interpreted vs. compiled)
- ✅ Within 2x of Spin's memory usage with collapse compression
- ✅ Support models with up to 10M states (with bitstate mode)

### Integration

- ✅ Library API supports all verification modes (DFS, BFS, POR, LTL)
- ✅ CLI accepts Spin-compatible flags
- ✅ Trail format readable by `spin -t` (or documented conversion tool)

## Scope

### In Scope (v2.0)

| Feature | Priority | Notes |
|---------|----------|-------|
| LTL → Büchi conversion | 🔴 Critical | Use omega-automata crate |
| Product construction (model × Büchi) | 🔴 Critical | Nested DFS on product space |
| POR C3 cycle condition | 🔴 Critical | Required for soundness |
| Collapse compression | 🔴 Critical | Per-process component grouping |
| d_step support | 🟡 Important | Promela parity |
| Remote references (P@x) | 🟡 Important | Promela parity |
| Weak/strong fairness | 🟡 Important | Property expressiveness |
| Spin binary trail format | 🟡 Important | Compatibility |
| Never claim improvements | 🟡 Important | Alternative to LTL |

### Out of Scope (v2.x or later)

| Feature | Rationale |
|---------|-----------|
| Embedded C code (`c_code {}`) | Conflicts with "no GCC" value; security risk |
| Parallel verification | High complexity, not required for parity |
| Stubborn sets | Optimization, not correctness |
| Priority scheduling | Niche feature, low priority |
| Multi-core state exploration | Performance optimization |

### Explicitly Defered to v2.1+

- Parallel DFS/BFS (multi-threading)
- Advanced POR (stubborn sets, target sets)
- Distributed verification (multi-machine)
- Incremental verification (model changes)

## Non-Goals

- **Faster than Spin**: Performance parity is sufficient; correctness is the priority
- **More features than Spin**: Match Spin's feature set, don't exceed it (yet)
- **Support non-Promela input**: Promela is the input language (as in v1)
- **Replace Spin for all users**: Target users who need embeddability or Rust integration

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| LTL → Büchi pipeline too slow | Performance degradation | Benchmark early; cache automata; optimize hot paths |
| C3 condition breaks existing POR | Regression in state-space size | Test against Spin's POR results; add regression tests |
| Collapse implementation too complex | Delayed timeline | Start with simple per-process grouping; iterate |
| omega-automata API incompatibility | Blocked on dependency | Fork omega-automata if needed; wrap in abstraction layer |
| Trail format incompatibility | Users can't use `spin -t` | Document conversion tool; consider native binary format |

## Validation Strategy

### Benchmark Suite

- Peterson's mutual exclusion (2-10 processes)
- Leader election protocols
- Communication protocols (alternating bit, sliding window)
- Classic Spin examples from distribution

### Comparison Testing

- Run identical models through Spin 6.5.x and spin-rs v2
- Compare: states explored, transitions, errors found, trails
- Acceptable: Within 5x performance, identical verification results

### Property Coverage

- Test all LTL operators: `[]`, `<>`, `X`, `U`, `V`, `->`, `&&`, `||`, `!`
- Test nested formulas: `[]<>p`, `<>(p U q)`, `[](p -> <>q)`
- Test fairness constraints: weak fairness, strong fairness

## Timeline (Target)

| Phase | Duration | Deliverables |
|-------|----------|--------------|
| Phase 1: LTL → Büchi | 2-3 weeks | omega-automata integration, product construction, nested DFS |
| Phase 2: POR C3 | 1-2 weeks | Cycle detection, sound ample-set selection |
| Phase 3: Collapse | 2-3 weeks | Per-process compression, memory benchmarks |
| Phase 4: Parity Features | 3-4 weeks | d_step, remote refs, fairness, trail format |
| Phase 5: Validation | 2 weeks | Benchmark suite, comparison testing, documentation |

**Total: 10-14 weeks** (flexible based on complexity discoveries)

## Stakeholders

- **Primary users**: VeriPlan (needs LTL correctness), Rust developers embedding model checking
- **Secondary users**: Researchers needing Promela verification without GCC
- **Maintainers**: Need clean architecture, testable components, documented APIs

## Related Changes

- **Predecessor**: `promela-lua-verifier` (v1 foundation)
- **Dependencies**: omega-automata crate (already in deps, unused)
- **Consumers**: VeriPlan integration (future change)

## Open Questions

1. **Embedded C**: Should we support it via Lua FFI, or explicitly reject it as out-of-scope?
2. **Trail format**: Binary compatibility with Spin, or JSON-only with conversion tool?
3. **Fairness**: How critical is fairness support for target users?
4. **Performance budget**: What's the acceptable slowdown vs. Spin for correctness gains?

---

**Next Steps**:

1. ✅ Create this proposal
2. ⏳ Create design.md with architecture details
3. ⏳ Create specs for each feature (LTL, POR, Collapse, etc.)
4. ⏳ Create tasks.md with phased implementation plan
5. ⏳ Begin Phase 1 implementation

**Status**: **PROPOSED** — awaiting review
