## 1. Parser Implementation

- [x] 1.1 Add `TopLevel::ChannelArray { name: String, size: i64, line: usize }` variant to AST
- [x] 1.2 Add `chan_array_decl` parser function for `chan name[N];` syntax
- [x] 1.3 Update `top_level()` to include `chan_array_decl` before `var_decl`
- [x] 1.4 Update `send_stmt` parser to accept `expr` instead of `ident` for channel
- [x] 1.5 Update `recv_stmt` parser to accept `expr` instead of `ident` for channel
- [x] 1.6 Add AST support for channel array indexing in send/recv (channel field becomes expression-based)

## 2. Codegen Implementation

- [x] 2.1 Add `ChannelArray` handling in codegen match statement
- [x] 2.2 Emit N state variables for channel array: `state.name_0 = nil`, `state.name_1 = nil`, ...
- [x] 2.3 Update `Send` statement codegen to handle expression-based channel (compute name dynamically)
- [x] 2.4 Update `Recv` statement codegen to handle expression-based channel (compute name dynamically)
- [x] 2.5 Generate Lua code for indexed access: `"name_" .. tostring(index)`

## 3. Runtime Implementation

- [x] 1.1 Add `ChannelArray` case in `from_model()` match statement
- [x] 1.2 Register N individual channels: `name_0`, `name_1`, ..., `name_N-1`
- [x] 1.3 All channels in array SHALL have capacity 0 (rendezvous)
- [x] 1.4 Add runtime bounds checking for indexed access (panic with clear error if out of bounds)
- [x] 1.5 Verify indexed send/recv operations work with dynamically computed channel names

## 4. Testing

- [x] 4.1 Add parser unit test: `test_chan_array_decl()` - verify AST node produced
- [x] 4.2 Add parser unit test: `test_chan_array_indexed_send()` - verify `tok[i] ! msg` parses
- [x] 4.3 Add parser unit test: `test_chan_array_indexed_recv()` - verify `tok[i] ? msg` parses
- [x] 4.4 Add codegen unit test: verify N state variables emitted for array
- [x] 4.5 Add runtime unit test: verify N channels registered
- [x] 4.6 Add integration test: `test_channel_array_token_ring()` - 3-node token ring with channel array
- [x] 4.7 Verify all existing channel tests still pass (backward compatibility)

## 5. Benchmark Integration

- [x] 5.1 Verify `token_ring_n5` model parses without errors
- [~] 5.2 Run `token_ring_n5` benchmark and compare state count with Spin (blocked: needs `for ... in` syntax support)
- [~] 5.3 Remove any skip markers for `token_ring_n5` in benchmark suite (blocked: depends on 5.2)
- [x] 5.4 Document any remaining limitations in benchmark README or comments
- [x] 5.5 Mark tasks 5.2 and 5.3 as deferred to future change (for-loop syntax support)

## 6. Documentation

- [x] 6.1 Update change `design.md` with any implementation learnings
- [x] 6.2 Add "Known Limitations" section if any features deferred (e.g., buffered channel arrays)
- [x] 6.3 Update README if channel arrays are a notable new capability
