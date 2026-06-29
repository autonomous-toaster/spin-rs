## Context

spin-rs's benchmark suite has 12 models from the VeriPlan corpus. The current state: 6 out of 36 benchmark configs pass (all from deadlock_circular and token_ring_n5, which happen to work by coincidence). Most models explore 1-2 states instead of the expected 20-114. The root causes span three layers:

1. **Codegen** — local variable writes target wrong Lua keys, arrays emit as scalars, for loops emit broken code, inline definitions are never expanded
2. **Runtime** — deadlock detection flags false positives on normal sequential termination, rendezvous channels don't block
3. **Engine** — LTL verification is a no-op stub, never wired into the checker pipeline

Each fix is self-contained and affects a specific code path. None require architectural changes — they're bugs in existing but broken feature implementations.

## Goals / Non-Goals

**Goals:**

- Fix all 9 identified bugs across codegen, runtime, and engine
- Achieve ~30/36 benchmark pass rate (up from 6/36)
- All 12 models explore non-trivial state spaces (not just 1-2 states)
- LTL verification actually runs for models with LTL formulas
- Inline definitions expand correctly at codegen time
- Arrays initialize as Lua tables for proper element access
- Deadlock detection only fires on genuine deadlocks

**Non-Goals:**

- No new features or capabilities beyond bug fixes
- No POR integration (standalone `check_dfs_por_with_c3` exists but is not wired into Checker)
- No ltl2ba correctness improvements (simplified ltl2ba is sufficient for benchmark formulas)
- No trail format compatibility, CLI parity, fairness, collapse compression
- No full rendezvous pairing (the flat transition model doesn't support send/recv as a single atomic step)

## Decisions

**D1: Local variable prefixing in guard effects.**

Problem: `emit_guards` (effects.rs) inlines assignment effects as `s.{target} = {value}` but doesn't prefix `target` with the proctype name — unlike `emit_assignment` (stmts.rs) which does.

Chosen: Add the `current_proctype → target_name` prefixing logic from `emit_assignment` to both:

- The inline assignment path inside `emit_guards` (the main bug)
- `emit_assignment_effect` (used in atomic/d_step blocks, same bug)

Alternative: Don't prefix local vars at all and store everything un-prefixed. Rejected because global and local variables from different proctypes would collide.

**D2: Deadlock detection — per-process done tracking.**

Problem: `check_violation` parses `_nr_pr` and checks for `:false` + `_done_` anywhere in the blob. This catches normal termination (process A finishes → _done_A = true, process B still running → the blob has both `_done_` and `:false` somewhere).

Chosen: Modify the deadlock check to:

1. Count processes where `_done_<name>` is `false` (still running)
2. Only flag deadlock if count >= 1 AND zero transitions

Alternative: Track active process count in the state blob (`_nr_pr_active`). Rejected — parsing `_done_` flags is more direct and doesn't need state blob changes.

**D3: Array initialization as Lua tables.**

Chosen: When `VarDecl.array_size` is `Some(n)` and `n > 0`, emit `state.{name} = {0, ..., 0}` (Lua table with n zero elements). Array access `{name}[{index}]` in expressions already maps to Lua's `s.{name}[index+1]` (Promela is 0-indexed, Lua is 1-indexed).

Alternative: Store arrays as flat scalars and offset all access logic. Rejected — Lua tables are natural for Promela arrays and the expression codegen already handles them.

**D4: For loop expansion at parse time.**

Chosen: Expand `for (var in start .. end) { body }` into sequential statements at parse time: `var = start; body; var = start+1; body; ... var = end; body`. This is simpler than fixing the codegen's complex loop iteration and covers all benchmark uses (initialization loops with small bounds).

Alternative: Fix codegen to emit a Lua while loop with a loop variable. Rejected because for loops in Promela can contain mutable loop variables, break, etc. — sequential expansion preserves semantics exactly and is high-confidence.

**D5: LTL wiring — integrate into checker post-exploration.**

Chosen: After DFS/BFS exploration completes, if the model has LTL formulas, run `PropertyChecker::check_liveness()` which builds the product automaton (model × ¬LTL) and runs nested DFS. Report violations alongside safety violations.

Alternative: Run product construction interleaved with DFS. Rejected — simpler to run a separate pass; product construction builds the Büchi automaton from the already-explored state graph.

**D6: Inline expansion at codegen time.**

Chosen: Before emitting proctype code, collect all `TopLevel::Inline` definitions into a HashMap. When encountering `Stmt::Run(name, args, _)` or a guard that calls an inline function, look up the definition, substitute parameters (by position), and emit the body directly. For the dining_n4 case: `pickup(i)` expands to `atomic { (fork[i] == 0); fork[i] = 1 }`.

Alternative: Expand in the parser/AST before codegen. Rejected — codegen already has the right context for variable scoping, and inline bodies need access to the caller's variable scope.

**D7: Rendezvous channels — simple fix with documented limitation.**

Chosen: Modify `LuaChannel::send` so capacity-0 (rendezvous) always returns `false` (never available to send alone). This means rendezvous sends will never appear as enabled transitions — models that depend on rendezvous for progress will deadlock (which is correct: both sides must be ready). Full rendezvous pairing (send+recv as a single atomic action) is deferred.

Alternative: Implement true rendezvous pairing in the transition model. Rejected — this requires significant architecture changes to support atomic send+recv pairs across proctypes.

## Risks / Trade-offs

**[Risk] For loop expansion at parse time breaks for large bounds.**
→ Mitigation: Benchmark bounds are small (0..4, 0..100). Sequential expansion is O(bound) in code size. Acceptable for now; document that for loops with large bounds should use runtime iteration.

**[Risk] Inline expansion at codegen time may duplicate code.**
→ Mitigation: Inline calls in Promela are typically small (1-3 statements). The dining_n4 case has 2-3 lines per inline. Code size increase is negligible.

**[Risk] LTL verification may produce false negatives for complex formulas.**
→ Mitigation: The simplified ltl2ba is sufficient for benchmark LTL formulas (`[](x==0)`, `[](p -> q)`, `[]<>p`). Full ω-automata correctness is deferred.

**[Risk] Rendezvous fix breaks models that depend on unpaired sends.**
→ Mitigation: No benchmark models depend on unpaired rendezvous sends. The current behavior (capacity-0 = unbounded) is incorrect. Fixing it makes channels consistent with Promela semantics.

## Migration Plan

No migration needed — this is a bug-fix change to an existing codebase. All changes are:

1. Fix codegen bug (one file, one scoped change)
2. Fix runtime bug (one file, one scoped change)
3. Test by running benchmark suite

No schema changes, no data migrations, no API breaks.

## Open Questions

1. **For loop iteration variable**: Should `for (i in 0 .. 4)` scope `i` to the loop body, or leak it? Current parse-time expansion doesn't scope it. Resolved: match Spin's behavior, which leaks the loop variable.
