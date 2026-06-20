## Context

12 benchmark models, only 1 fully working. The root causes are feature gaps in the parser, codegen, and runtime. This change fills those gaps phase by phase, prioritizing deadlock detection and channel support.

### Current Architecture for Channels

The runtime already has `LuaChannel` with send/recv capabilities. The parser recognizes `chan` as a variable type (`VarType::Chan`). But the pipeline is broken:

1. `chan ch = [0] of { byte }` is parsed as `var_decl` → `TopLevel::GlobalVar(VarDecl { var_type: Chan, init: Some(...) })`. The capacity `[0]` and type `{ byte }` are embedded in the init expression.
2. The runtime's `from_model` checks for `TopLevel::ChanDecl` — a variant that's never produced by the parser (dead code).
3. The codegen emits `state.ch = nil` (default for Chan), but sends/recvs reference the channel by name via the Rust FFI.

### Current Architecture for Deadlocks

The `LuaModel::check_violation` returns `None` (default trait implementation). There's no deadlock detection. In Spin, a deadlock occurs when every process is blocked or terminated and at least one process hasn't reached end state. The fix:

- After enumerating transitions, if `transitions.len() == 0` AND there are active processes, flag as deadlock
- Track whether each proctype has reached its end state via `_done_<name>` flag

## Decisions

**D1: `active [N]` → N synchronous processes.**
Chosen: Generate N identical proctype definitions with distinct names. Alternative: share one definition with N instances. The flat approach matches the current single-definition architecture.

**D2: Multi-variable decls via expansion.**
Chosen: Parse `bool a, b, c;` and expand to multiple `VarDecl` nodes at parse time. Alternative: add a new AST node. Expansion is cleaner for downstream consumers.

**D3: Channels as Rust-side state only.**
Chosen: Channel buffers live in Rust (Arc<Mutex<HashMap<...>>>), not in the Lua state table. The state blob only tracks the channel name. Sends/recvs mutate the channel state directly via FFI calls. Alternative: put channel state in the Lua table for snapshotting — too complex for now.

## Risks

**[Risk] State hashing without channel content:** If channels live in Rust, two states with different channel contents but the same state blob hash to the same value. Spin includes channel state in the state vector.
→ *Mitigation*: Include the channel's message count hash in the state blob. For Phase 1, accept the limitation (rendezvous channels are always empty).

**[Risk] Rendezvous matching:** Capacity-0 channels require synchronous sender-receiver pairing. The flat transition model evaluates transitions independently.
→ *Mitigation*: Treat capacity-0 channels as always-enabled send/recv. The effect performs the operation; if the other side isn't ready, it fails silently. This won't detect deadlocks correctly — mark as known limitation for Phase 1.

## Known Limitations

The following limitations are accepted for this change and will be addressed in follow-up work:

### L1: Channel Capacity Not Extracted

**Issue**: When channels are parsed as `GlobalVar(VarDecl { var_type: Chan })`, the capacity `[N]` is embedded in the init expression but never extracted.

**Impact**: All channels default to capacity 0 (rendezvous), even if declared as `chan ch = [5] of { byte };`.

**Workaround**: Use only rendezvous channels in models. Buffered channels will behave as rendezvous.

**Follow-up**: `fix-channel-arrays` change addresses proper channel capacity extraction.

### L2: Channel Arrays Not Supported

**Issue**: The syntax `chan name[N];` (channel array declaration) is not implemented. Parser expects `chan name = [N] of { type };`.

**Impact**: Models using channel arrays (e.g., `token_ring_n5` with `chan tok[5];`) will fail to parse.

**Workaround**: Use individual channel declarations: `chan tok0; chan tok1; ...`

**Follow-up**: `fix-channel-arrays` change implements channel array support.

### L3: Channel Message Type Not Enforced

**Issue**: The `{ type }` in `chan ch = [N] of { type }` is never extracted or enforced.

**Impact**: No compile-time or runtime type checking on channel messages.

**Workaround**: Manually ensure type correctness in models.

**Follow-up**: Future change may add channel type extraction and enforcement.

### L4: Multi-Variable Declarations Untested

**Issue**: `VarDecls` AST variant exists but no parser function creates it. No tests verify `bool a, b, c;` produces multiple declarations.

**Impact**: Unknown if multi-var declarations work correctly.

**Workaround**: Use separate declarations: `bool a; bool b; bool c;`

**Follow-up**: Add tests and verify implementation if benchmarks use this syntax.
