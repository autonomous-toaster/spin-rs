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

- [ ] 2.1 CFG builder per proctype: nodes = positions between statements, edges = statements
- [ ] 2.2 Emit one transition per edge, guarded on the position, effect carrying the successor
- [ ] 2.3 Collapse the five counters into one `_pc_<P>`; derive termination from it
- [ ] 2.4 `atomic` and `d_step` as one fused edge per region
- [ ] 2.5 `goto`, `label`, `break` as edge targets; no control transfer writes the termination flag
- [ ] 2.6 `for` as init edge, loop-head node and exit edge
- [ ] 2.7 Sequencing sweep: statement count x kind x observed-write position matches pan for every
      generated shape
- [ ] 2.8 Remove the shape-specific repairs the CFG makes unnecessary, one at a time, each
      gated by 1.5: nested-if expansion, trailing-`break` fold, multi-statement branch refusal,
      blocking-expression hoisting guard
- [ ] 2.9 Delete the old emitters last, after 2.7 and 1.5 are green on the new path

## Phase 3: Statement Semantics

- [ ] 3.1 `else` guard resolved against the other guards of its own selection, not against a constant
- [ ] 3.2 `unless` escape edge: handler taken only when the body cannot move
- [ ] 3.3 `timeout` enabled only when no process can move
- [ ] 3.4 `provided` clause enforced (parsed today, referenced nowhere outside the AST)
- [ ] 3.5 `d_step` restrictions: no blocking, no nondeterminism, else refuse loudly
- [ ] 3.6 Deadlock means invalid end state: a state with no transitions is an error only when some
      process is not at a valid end state
- [ ] 3.7 Fairness survey: whether `fair` or `-f` changes a verdict on any supported model.
      Human review only; never silently ignored

## Phase 4: Data Surface

- [ ] 4.1 Integer types and wraparound: `bit`, `byte`, `short`, `int`, `unsigned`, `mtype` ranges and overflow
- [ ] 4.2 Arrays, including multi-dimensional and indexed writes
- [ ] 4.3 `typedef` and structured fields in the state
- [ ] 4.4 Remote references `P[i]:x`, and `_pid`, `_last`, `_nr_pr`
- [ ] 4.5 Channel surface: rendezvous, sorted send, random receive, poll, eval, `len/empty/full/nempty/nfull`,
      channel arrays

## Phase 5: Property Surface

- [ ] 5.1 `Until` and `Release` added to the formula type and to the translation (absent from the type today)
- [ ] 5.2 Nested temporal operators, starting with `<>[]p` and `[]<>p`
- [ ] 5.3 Atom evaluation over the state, so a comparison is decided rather than declined
- [ ] 5.4 `never` claims: verdict parity against pan
- [ ] 5.5 `progress`, `accept` and `end` labels; non-progress cycles
- [ ] 5.6 Liveness verdict parity on the generated corpus
- [ ] 5.7 A model that terminates SHALL NOT be reported as an accepting cycle: `b = 1; a = 1` with
      `<>(a)` is refuted today and pan accepts it

## Phase 6: Inert Mechanisms

- [ ] 6.1 Audit every AST field, emitted state key and CLI flag for participation: parsed or
      written but never read is a defect. `provided` and `_pc_` were both in this class
- [ ] 6.2 A test that fails when a state key is initialised and read by nothing, so the next one is
      caught mechanically rather than by a reader noticing
