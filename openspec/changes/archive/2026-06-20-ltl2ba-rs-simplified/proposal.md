## Why

spin-rs v2 requires full LTL verification via Büchi automata construction. The omega-automata crate provides LTL → Büchi conversion but doesn't expose the resulting automaton structure publicly, blocking integration. Implementing a complete LTL → Büchi converter (like ltl2ba) requires 4-6 weeks of complex automata-theory code. This change creates a **simplified LTL → Büchi converter** that supports the most common LTL operators ([]p, <>p, Xp) immediately, with a clear migration path to full LTL support in a future version.

## What Changes

- **New crate**: `ltl2ba-rs-simplified` — Rust implementation of simplified LTL → Büchi conversion
- **Supported operators**: `[]` (always), `<>` (eventually), `X` (next), `!` (negation), `&&` (conjunction), `||` (disjunction)
- **Unsupported operators** (documented with clear errors): `U` (until), `V` (release), nested temporal operators
- **Integration**: spin-rs property module uses ltl2ba-rs-simplified for LTL verification
- **Documentation**: Clear "simplified" labeling, coverage estimates (~60-70% of real-world properties)
- **Migration path**: Full ltl2ba implementation can replace simplified version without API changes

## Capabilities

### New Capabilities

- `ltl-parser`: LTL string parsing into formula AST (supports all standard operators, rejects unsupported with clear errors)
- `buchi-construction`: Simplified Büchi automaton construction for []p, <>p, Xp patterns
- `product-construction`: Model state × Büchi state product for LTL verification
- `nested-dfs`: Accepting cycle detection in product space for liveness violations

### Modified Capabilities

- `property-engine`: Extends existing property verification to use Büchi automata (currently uses simplified nested DFS without proper Büchi construction)

## Impact

- **Affected code**: `src/property/mod.rs`, `src/property/buchi.rs` (new module for ltl2ba-rs-simplified)
- **Dependencies**: Adds `ltl2ba-rs-simplified` crate (internal, can be extracted later)
- **APIs**: `BuchiAutomaton::from_ltl()` changes from stub to working implementation
- **Limitations**: Users needing `U`/`V` operators must wait for full implementation or use Spin directly
- **Performance**: Simplified automata are small (1-3 states), negligible overhead vs. full tableau
