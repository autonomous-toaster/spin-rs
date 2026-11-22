# Tasks

## Phase 1: The Instrument

- [x] 1.1 Structural model generator: body-shape grammar (statement count x kind x observed-write
      position x nesting depth x process count x claim shape), emitting Promela plus claims
- [x] 1.2 Pan oracle runner over generated models, classifying each as agree, false refutation,
      false clean, decline (`UnverifiedKind` named), refusal, or pan-unbuildable
- [x] 1.3 Baseline recorded in `baseline.md` and `known-wrong.txt`. Generated corpus: 31 agree,
      25 false refutation, 5 false clean, 4 refusal of 65. Hand corpus at `ca86943`: 67 agree,
      3 false refutation, 1 false clean, 8 decline, 1 refusal of 83
- [x] 1.4 Mutation test for the generator: it regenerates `order`, `x_beforeseq`, `x_twodo` and
      `adv_loop_and_assert`; fails if any of the four shapes is unreachable from the grammar
- [x] 1.5 CI gate: a false refutation or false clean fails the build; declines are counted and
      printed, never subtracted from the total

## Phase 2: Execution Model

- [x] 2.1 CFG builder per proctype: nodes = positions between statements, edges = statements
      (`src/codegen/flatten.rs`: layout then emit, every jump resolved at generation time)
- [x] 2.2 Emit one transition per edge, guarded on the position, effect carrying the successor
- [ ] 2.3 Collapse the five counters into one `_pc_<P>`; derive termination from it
- [x] 2.4 `atomic` and `d_step` as one fused edge per region
- [x] 2.5 `goto`, `label`, `break` as edge targets; no control transfer writes the termination flag
- [x] 2.6 `for` as init edge, loop-head node and exit edge
- [x] 2.7 Sequencing sweep: the generated corpus measures this. Sequences, an assignment before
      a loop and two sequential loops no longer produce a wrong verdict
- [x] 2.10 Nested `do/od` as a step
- [x] 2.11 `for` loops as steps. The construct was wrong in three ways beyond the refusal:
      it was expanded to an `atomic` region (so an intermediate write was unobservable, which
      hid a refutation pan finds), it did not absorb a trailing semicolon (so a `for` followed by
      another statement took the whole proctype down with it), and it accepted only `in` where
      Promela writes `:`. pan is unambiguous that the body is not atomic, so the loop is laid
      out as init/condition/update
- [x] 2.12 `atomic` and `d_step` as ONE fused step
- [x] 2.13 A nested `if` whose branch is another `if`
- [ ] 2.8 Remove the shape-specific repairs the CFG makes unnecessary, one at a time, each
      gated by 1.5: nested-if expansion, trailing-`break` fold, multi-statement branch refusal,
      blocking-expression hoisting guard
- [ ] 2.9 Delete the old emitters last, after 2.7 and 1.5 are green on the new path
- [x] 2.14 Stop the step machine from swallowing a statement it cannot express, and stop
      `verify()` from ignoring an evaluation failure: both were silent wrongs, not refusals

## Phase 3: Statement Semantics

- [x] 3.1 `else` guard resolved against the other guards of its own selection, not against a constant.
      A guard whose condition the parser did not capture is either `else` or a statement standing in
      for one; only a blocking statement supplies a guard, which is how `:: c ? x ->` is expressed
- [ ] 3.2 `unless` escape edge: refused today. The handler interleaves with the body, which needs a
      second step counter per process and a transition over both; the construct is also written as
      `{ body } unless { handler }`, and a bare block is not a statement the parser has
- [ ] 3.3 `timeout` enabled only when no process can move
- [x] 3.4 `provided` clause enforced: the condition gates every transition of its proctype.
      It was parsed and referenced nowhere outside the AST, and once the parser accepted it that
      showed up as a false refutation
- [ ] 3.5 `d_step` restrictions: no blocking, no nondeterminism, else refuse loudly
- [ ] 3.6 Deadlock means invalid end state: a state with no transitions is an error only when some
      process is not at a valid end state
- [ ] 3.7 Fairness survey: whether `fair` or `-f` changes a verdict on any supported model.
      Human review only; never silently ignored

- [ ] 3.8 `run` refused: a spawned process needs dynamic instances, and the transition table is
      emitted per static instance
- [ ] 3.9 `priority` parsed and recorded as unimplemented; it is not silently ignored, but it is not
      enforced either
- [ ] 3.10 A `{ ... }` block is not a statement: `unless`, and the grouping form on its own, do not parse
- [x] 3.11 A guard is true when it is non-zero, as Promela says and as Lua does not: 0, `false` and `nil`
      are false, everything else true. Four probe models were refuted by a `(n)` guard with `n == 0`
- [x] 3.12 `->` is a statement separator, and `x++`/`x--` are statements; neither parsed

## Phase 4: Data Surface

- [x] 4.1 Integer types and wraparound: every stored value is truncated to its declared type, and
      `/` and `%` follow C rather than Lua. Four probe models were clean where pan refutes them before
      this. `unsigned` widths and `mtype` ranges are covered by the same truncation
- [ ] 4.2 Arrays, including multi-dimensional and indexed writes
- [ ] 4.3 `typedef` and structured fields in the state
- [ ] 4.4 Remote references `P[i]:x`, and `_pid`, `_last`, `_nr_pr`
- [ ] 4.5 Channel surface: rendezvous, sorted send, random receive, poll, eval, `len/empty/full/nempty/nfull`,
      channel arrays
- [x] 4.6 A state SHALL determine its own transitions. A channel's queue lived in a registry shared
      by the runtime and keyed by name, so reopening an earlier state saw the messages a later
      one had left behind: `_spin_chan_send` wrote the registry and mirrored a copy into the
      state, while `_full`/`_len`/`_empty` read only the registry. Only capacity still comes
      from the declaration, because capacity is static. This was a soundness hole, not a display
      bug: it turned a terminating model into an "accepting cycle" at trail length 0

## Phase 5: Property Surface

- [ ] 5.1 `Until` and `Release` added to the formula type and to the translation (absent from the type today)
- [ ] 5.2 Nested temporal operators, starting with `<>[]p` and `[]<>p`
- [ ] 5.3 Atom evaluation over the state, so a comparison is decided rather than declined
- [ ] 5.4 `never` claims: verdict parity against pan
- [ ] 5.5 `progress`, `accept` and `end` labels; non-progress cycles
- [ ] 5.6 Liveness verdict parity on the generated corpus
- [x] 5.7 A model that terminates SHALL NOT be reported as an accepting cycle. The last case of
      this was not about termination at all, and the state could not answer for it. See 4.6
- [x] 2.15 A `for` followed by another statement failed to parse; it was the only statement
      parser that did not absorb an optional trailing semicolon
- [x] 2.16 `<>` over a model that sends before the observed write was refuted where pan accepts.
      The cause was 4.6, not the liveness search

## Phase 6: Inert Mechanisms

- [ ] 6.1 Audit every AST field, emitted state key and CLI flag for participation: parsed or
      written but never read is a defect. `provided` and `_pc_` were both in this class
- [ ] 6.2 A test that fails when a state key is initialised and read by nothing, so the next one is
      caught mechanically rather than by a reader noticing
