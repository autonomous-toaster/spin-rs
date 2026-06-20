## Why

The `token_ring_n5` benchmark model cannot be parsed because channel array syntax (`chan tok[5];`) and channel array indexing (`tok[i] ! msg`) are not implemented. This blocks benchmark completion and prevents testing of multi-channel coordination patterns.

## What Changes

- **Parser**: Add support for `chan name[N];` syntax (channel array declarations)
- **Parser**: Update send/recv statements to accept expressions (not just identifiers) for channel access
- **AST**: Add `ChannelArray` variant to `TopLevel` enum
- **Codegen**: Emit channel array initialization code for N channels
- **Runtime**: Register N individual channels from array declaration
- **Runtime**: Support indexed channel access in send/recv operations

## Capabilities

### New Capabilities

- `channel-arrays`: Support for Promela channel array syntax (`chan name[N];`) and indexed access (`chan[i] ! msg`)

### Modified Capabilities

- `channel-support`: Extends existing channel support to include array declarations and indexed access

## Impact

- `src/parser/mod.rs`: New `chan_array_decl` parser, update `send_stmt`/`recv_stmt` to accept expressions
- `src/parser/ast.rs`: Add `TopLevel::ChannelArray` variant
- `src/codegen/mod.rs`: Handle `ChannelArray`, emit array initialization
- `src/runtime/mod.rs`: Register N channels, support indexed access
- `benches/bench_vs_spin.rs`: Enable `token_ring_n5` benchmark (remove skip if present)
- `tests/integration.rs`: Add channel array tests

**Dependencies**: None - this is a self-contained feature addition

**Breaking Changes**: None - purely additive functionality
