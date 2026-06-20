## 1. Parser Implementation

- [ ] 1.1 Add `TopLevel::ChannelArray { name: String, size: i64, line: usize }` variant to AST
- [ ] 1.2 Add `chan_array_decl` parser function for `chan name[N];` syntax
- [ ] 1.3 Update `top_level()` to include `chan_array_decl` before `var_decl`
- [ ] 1.4 Update `send_stmt` parser to accept `expr` instead of `ident` for channel
- [ ] 1.5 Update `recv_stmt` parser to accept `expr` instead of `ident` for channel
- [ ] 1.6 Add AST support for channel array indexing in send/recv (channel field becomes expression-based)

## 2. Codegen Implementation

- [ ] 2.1 Add `ChannelArray` handling in codegen match statement
- [ ] 2.2 Emit N state variables for channel array: `state.name_0 = nil`, `state.name_1 = nil`, ...
- [ ] 2.3 Update `Send` statement codegen to handle expression-based channel (compute name dynamically)
- [ ] 2.4 Update `Recv` statement codegen to handle expression-based channel (compute name dynamically)
- [ ] 2.5 Generate Lua code for indexed access: `"name_" .. tostring(index)`

## 3. Runtime Implementation

- [ ] 1.1 Add `ChannelArray` case in `from_model()` match statement
- [ ] 1.2 Register N individual channels: `name_0`, `name_1`, ..., `name_N-1`
- [ ] 1.3 All channels in array SHALL have capacity 0 (rendezvous)
- [ ] 1.4 Add runtime bounds checking for indexed access (panic with clear error if out of bounds)
- [ ] 1.5 Verify indexed send/recv operations work with dynamically computed channel names

## 4. Testing

- [ ] 4.1 Add parser unit test: `test_chan_array_decl()` - verify AST node produced
- [ ] 4.2 Add parser unit test: `test_chan_array_indexed_send()` - verify `tok[i] ! msg` parses
- [ ] 4.3 Add parser unit test: `test_chan_array_indexed_recv()` - verify `tok[i] ? msg` parses
- [ ] 4.4 Add codegen unit test: verify N state variables emitted for array
- [ ] 4.5 Add runtime unit test: verify N channels registered
- [ ] 4.6 Add integration test: `test_channel_array_token_ring()` - 3-node token ring with channel array
- [ ] 4.7 Verify all existing channel tests still pass (backward compatibility)

## 5. Benchmark Integration

- [ ] 5.1 Verify `token_ring_n5` model parses without errors
- [ ] 5.2 Run `token_ring_n5` benchmark and compare state count with Spin
- [ ] 5.3 Remove any skip markers for `token_ring_n5` in benchmark suite
- [ ] 5.4 Document any remaining limitations in benchmark README or comments

## 6. Documentation

- [ ] 6.1 Update change `design.md` with any implementation learnings
- [ ] 6.2 Add "Known Limitations" section if any features deferred (e.g., buffered channel arrays)
- [ ] 6.3 Update README if channel arrays are a notable new capability
