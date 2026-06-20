## Context

The `token_ring_n5` benchmark model uses Promela channel array syntax that is currently unsupported:

```promela
chan tok[5];           // Array of 5 rendezvous channels
init { 
    byte i; 
    for (i in 0 .. 4) { tok[i] = [1] of { byte } }; 
    tok[0] ! 1 
}
active [5] proctype node() {
    byte msg;
    do :: tok[_pid] ? msg -> ... od
}
```

Current implementation limitations:

- Parser: `send_stmt` and `recv_stmt` only accept `ident` for channel, not `expr` (e.g., `tok[i]`)
- Parser: No support for `chan name[N];` array declaration syntax
- AST: No `ChannelArray` variant
- Runtime: Cannot register or access channel arrays

**Constraints:**

- Must maintain backward compatibility with existing channel syntax
- Channel arrays should behave as N independent rendezvous channels
- Indexed access must work with expressions (`tok[i]`, `tok[_pid]`, `tok[0]`)

## Goals / Non-Goals

**Goals:**

- Parse `chan name[N];` as channel array declaration
- Parse `chan[i] ! msg` and `chan[i] ? var` with indexed access
- Generate correct Lua code for N channels
- Register N channels in runtime with indexed access
- Enable `token_ring_n5` benchmark to run successfully
- Support all expression types in index (literals, variables, `_pid`, arithmetic)

**Non-Goals:**

- Buffered channel arrays (`chan name[N] = [M] of { type };`) - out of scope
- Multi-dimensional channel arrays (`chan name[3][4];`) - out of scope
- Channel array initialization with non-default values - use explicit init pattern
- Type checking across array elements - all channels in array have same type

## Decisions

**D1: Channel array syntax: `chan name[N];`**

Chosen: Follow Promela standard syntax for channel arrays.

```promela
chan tok[5];    // Array of 5 channels (all rendezvous, capacity 0)
```

Alternative considered: `chan name = array[N]` - rejected, non-standard.

**D2: Channel type for arrays**

Chosen: All channels in array are rendezvous (capacity 0).

Rationale: Promela semantics for `chan name[N];` without `= [M] of {type}` is array of rendezvous channels. Buffered arrays would require explicit syntax.

**D3: Indexed access: expression-based**

Chosen: Channel position accepts any expression, not just literals.

```promela
tok[0] ! msg      // Literal index
tok[i] ! msg      // Variable index  
tok[_pid] ! msg   // Built-in variable
tok[i+1] ! msg    // Arithmetic expression
```

Implementation: Change `send_stmt` and `recv_stmt` parsers from `ident` to `expr` for channel field.

**D4: AST representation**

Chosen: Add `TopLevel::ChannelArray { name: String, size: i64, line: usize }`

Alternative: Reuse `VarDecl` with `array_size` - rejected, channels are top-level declarations, not variables.

**D5: Runtime channel registration**

Chosen: Register N individual channels with names `name_0`, `name_1`, ..., `name_N-1`.

Rationale: Matches `active [N]` proctype pattern. Indexed access `tok[i]` resolves to channel `tok_i` at runtime.

**D6: Codegen for channel arrays**

Chosen: Emit N channel registrations in init state:

```lua
state.tok_0 = nil
state.tok_1 = nil
state.tok_2 = nil
state.tok_3 = nil
state.tok_4 = nil
```

Runtime registers 5 separate channels: `tok_0`, `tok_1`, etc.

**D7: Indexed access codegen**

Chosen: Generate Lua code that computes channel name dynamically:

```lua
-- tok[i] ! msg becomes:
local _chan_name = "tok_" .. tostring(i)
runtime_send(_chan_name, {msg})
```

Alternative: Pre-compute channel references - rejected, too complex for initial implementation.

## Risks / Trade-offs

**[Risk] Expression evaluation order in indexed access**

If index expression has side effects, order matters. Example: `tok[f()] ! msg`

→ *Mitigation*: Document that index expressions should be side-effect free. Evaluate once per send/recv.

**[Risk] Performance of dynamic channel name computation**

Computing `"tok_" .. i` for every send/recv may be slower than direct channel reference.

→ *Mitigation*: Accept for initial implementation. Can optimize later with channel handle caching.

**[Risk] Out-of-bounds index access**

`tok[10]` when array size is 5 - undefined behavior.

→ *Mitigation*: Runtime check in `register_channel_array` - panic with clear error message if index out of bounds.

**[Risk] Interaction with state hashing**

Channel arrays add N state variables. State vector grows with array size.

→ *Mitigation*: Accept as inherent to model complexity. Large arrays = more states (expected).

**[Trade-off] Channel naming convention**

Using `tok_0`, `tok_1` naming means generated names are visible in error messages and state dumps.

→ *Acceptable*: Clear mapping between `tok[i]` and `tok_i`. Alternative (internal IDs) would be less debuggable.

## Migration Plan

**No migration needed** - this is additive functionality.

Existing models continue to work. New models can use channel arrays.

**Deployment steps:**

1. Implement parser changes (AST, grammar)
2. Implement codegen changes
3. Implement runtime changes
4. Add integration tests
5. Enable `token_ring_n5` in benchmark
6. Run full test suite

**Rollback:** Revert commit - no data migration or breaking changes.

## Open Questions

1. **Should channel arrays support explicit initialization?**
   - `chan tok[5] = [0] of { byte };` - array of 5 buffered channels?
   - Decision: Defer to future change. Current scope: rendezvous arrays only.

2. **Should we validate index bounds at parse time?**
   - `tok[10]` when declared as `chan tok[5];`
   - Decision: Runtime check only. Parse-time would require constant folding.

3. **How to handle channel arrays in `run` statements?**
   - `run P(tok)` - pass entire array?
   - `run P(tok[i])` - pass single channel?
   - Decision: Not supported in initial implementation. Document limitation.

## Implementation Learnings

- **Lua codegen for indexed channels**: Channel name expressions in Lua must include quotes at generation time. For simple channels, `channel_to_lua('tok')` returns `'tok'`. For indexed: `'tok_' .. tostring(i)`. Both are used directly in `chan_full()` / `chan_send()` calls without additional quoting.
- **`channel` field type changed from `String` to `Box<Expression>`**: This is a breaking AST change but maintains backward compatibility since simple identifiers are wrapped in `Expression::Ident`.
- **Array access parsed via `channel_expr()`**: A new parser combinator that first parses an ident, then optionally an `[expr]` index, producing either `Expression::Ident` or `Expression::ArrayAccess`.
- **Runtime bounds checking**: Uses existing "channel not found" error messages with enhanced text "(possible out-of-bounds array access)".
- **`token_ring_n5` benchmark**: Parses correctly but doesn't produce correct state exploration yet because: (1) `for ... in` syntax not supported, (2) `= [1] of { byte }` buffered init not supported, (3) `active [N] proctype` with `_pid` in `do...od` still generates 0 processes.
