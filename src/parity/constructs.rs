//! Verdicts on the constructs where this engine chooses between implementing and refusing.
//!
//! The models come from the consumer (veriplan), which wrote them to probe exactly those
//! constructs; the expected counts are pan's, measured here with `spin -a` plus `pan -a`. They
//! live in this repository as well as theirs on purpose: theirs is the guard on what they depend
//! on, this is the guard on what this engine claims.
//!
//! The contract is deliberately narrow. A construct may be refused or declined freely - that is
//! debt, and it is counted elsewhere. What must never happen is a model that is *decided* and
//! disagrees with pan, or a claim that is refuted while also reported undecided. Each entry below
//! starts as "no verdict" and asserts a verdict the moment the construct is implemented, which is
//! when a wrong answer would otherwise arrive silently.

use crate::parity::Ours;

pub struct Construct {
    pub name: &'static str,
    /// pan's error count, measured. `None` if pan cannot build the model.
    pub pan: Option<usize>,
    pub source: &'static str,
}

pub const CONSTRUCTS: &[Construct] = &[
    Construct {
        name: "never",
        pan: Some(1),
        source: "/* A never claim that matches every run: pan reports a violation. */\nbyte x = 0;\n\nactive proctype P() {\n    x = 1\n}\n\nnever { skip }\n",
    },
    Construct {
        name: "priority_none",
        pan: Some(1),
        source: "/* The same model without the clause, as the control: this one pan refutes. */\nbool flag = false;\n\nactive proctype Hi() {\n    do\n    :: true -> skip\n    od\n}\n\nactive proctype Lo() {\n    flag = true\n}\n\nltl p0 { [] ( !flag ) }\n",
    },
    Construct {
        name: "priority",
        pan: Some(0),
        source: "/* pan runs only the highest-priority enabled process, so Lo never runs here:\n   the claim holds. Without the priority clause the claim is refuted. */\nbool flag = false;\n\nactive proctype Hi() priority 2 {\n    do\n    :: true -> skip\n    od\n}\n\nactive proctype Lo() {\n    flag = true\n}\n\nltl p0 { [] ( !flag ) }\n",
    },
    Construct {
        name: "rendezvous_holds",
        pan: Some(0),
        source: "/* A capacity-0 channel with a claim pan decides: the send and the receive must meet, so got\n   becomes true and the liveness claim holds. */\nchan c = [0] of { bit };\nbool got = false;\n\nactive proctype Sender() { c ! 1 }\nactive proctype Receiver() { c ? got }\n\nltl p0 { <> ( got ) }\n",
    },
    Construct {
        name: "rendezvous_order",
        pan: Some(0),
        source: "/* Ordering through a rendezvous: the flag is only set after the handshake, so pan can decide\n   a claim about what must be true before it. */\nchan c = [0] of { bit };\nbool handshake = false;\nbool after = false;\n\nactive proctype Sender() { c ! 1; after = true }\nactive proctype Receiver() { c ? handshake }\n\nltl p0 { [] ( after -> handshake ) }\n",
    },
    Construct {
        name: "rendezvous_refutes",
        pan: Some(1),
        source: "/* The same model with the opposite claim: pan refutes, because got does become true. */\nchan c = [0] of { bit };\nbool got = false;\n\nactive proctype Sender() { c ! 1 }\nactive proctype Receiver() { c ? got }\n\nltl p0 { [] ( !got ) }\n",
    },
    Construct {
        name: "rendezvous",
        pan: Some(0),
        source: "/* Capacity-0 channel: send and receive must meet. */\nchan c = [0] of { bit };\nbool got = false;\n\nactive proctype Sender() { c ! 1 }\nactive proctype Receiver() { c ? got }\n\nltl p0 { [] ( got -> got ) }\n",
    },
    Construct {
        name: "run",
        pan: Some(0),
        source: "byte x = 0;\n\nproctype Worker() { x = 1 }\n\nactive proctype Main() {\n    run Worker();\n    skip\n}\n\nltl p0 { [] ( x <= 1 ) }\n",
    },
    Construct {
        name: "unless_refutes",
        pan: Some(1),
        source: "/* Same escape as unless.pml, but the claim is true only up to 2: the escape lets x reach 3,\n   so pan refutes. Together with unless.pml this brackets what the escape means. */\nbool stop = false;\nbyte x = 0;\n\nactive proctype P() {\n    do\n    :: x < 5 -> x++\n    od unless (x == 3)\n}\n\nltl p0 { [] ( x <= 2 ) }\n",
    },
    // The escape carried as a *handler with statements* rather than a bare guard. The handler's
    // guard is rendered to Lua before dependencies are declared, so its reads have to be carried
    // separately or the transition declares none - and then the state projection drops a field the
    // escape depends on, the escape never fires, and a claim about the handler's assignment reads
    // clean. v0.5.37 shipped exactly that: it accepted this model in 4 states where v0.5.35 and pan
    // refute it in 8. Reported by the consumer, whose fixture for a previous soundness hunt caught
    // it; these two are that fixture and its block-shaped pair.
    // An assertion's inputs are a finding's inputs. The construct is "assert", the position is
    // where its read is evaluated: body, block, loop, branch, label. v0.5.38 accepted all five
    // shapes before these were added, because the always-visible set held where a failure is
    // *recorded* but not what the check *reads*, and because _failed_assert appears only after a
    // failure so it was absent from the identity set built at initialisation.
    Construct {
        name: "assert_field_hidden_by_reduction",
        pan: Some(1),
        source: "/* x is written by A and read only by an assert, and the model has no claim. pan refutes\n   because B can observe x == 1 between A's two writes. */\nbyte x = 0;\nactive proctype A() { do :: x = 1; x = 2 od }\nactive proctype B() { assert(x != 1) }\n",
    },
    Construct {
        name: "assert_in_block_field",
        pan: Some(1),
        source: "/* The same assert nested in a bare block, which is where a container-walking omission\n   hides it. */\nbyte x = 0;\nactive proctype A() { do :: x = 1; x = 2 od }\nactive proctype B() { { assert(x != 1) } }\n",
    },
    Construct {
        name: "assert_in_do_field",
        pan: Some(1),
        source: "/* The assert one step into a do loop rather than first in the body. */\nbyte x = 0;\nactive proctype A() { do :: x = 1; x = 2 od }\nactive proctype B() { do :: (1) -> assert(x != 1); break od }\n",
    },
    Construct {
        name: "assert_in_if_field",
        pan: Some(1),
        source: "/* The assert inside an if branch. */\nbyte x = 0;\nactive proctype A() { do :: x = 1; x = 2 od }\nactive proctype B() { if :: (1) -> assert(x != 1) fi }\n",
    },
    Construct {
        name: "assert_behind_label",
        pan: Some(1),
        source: "/* The assert reached through a label jump. */\nbyte x = 0;\nactive proctype A() { do :: x = 1; x = 2 od }\nactive proctype B() { goto chk; chk: assert(x != 1) }\n",
    },
    Construct {
        name: "unless_handler_loop_refutes",
        pan: Some(1),
        source: "/* The escape is re-checked at the loop head with x already 3, so the handler runs and\n   y becomes 9. The claim y <= 2 is therefore refuted. */\nbyte x = 0;\nbyte y = 0;\n\nactive proctype P() {\n    do\n    :: true -> x = 1; x = 2; x = 3\n    od unless { (x == 3); y = 9 }\n}\n\nltl p0 { [] ( y <= 2 ) }\n",
    },
    Construct {
        name: "unless_handler_block_holds",
        pan: Some(0),
        source: "/* The block form of the same escape: x is already 3 when the block runs, so the handler\n   never fires and y stays 0. pan accepts it, which brackets the loop case above. */\nbyte x = 0;\nbyte y = 0;\n\nactive proctype P() {\n    { x = 1; x = 2; x = 3 } unless { (x == 3); y = 9 }\n}\n\nltl p0 { [] ( y <= 2 ) }\n",
    },
    Construct {
        name: "unless",
        pan: Some(0),
        source: "/* The escape fires at x == 3, so the loop cannot reach 5. */\nbool stop = false;\nbyte x = 0;\n\nactive proctype P() {\n    do\n    :: x < 5 -> x++\n    od unless (x == 3)\n}\n\nltl p0 { [] ( x <= 4 ) }\n",
    },
    // Reconstructed from the consumer's description of the pair that brackets what an
    // escape means: the handler is armed by the first statement of the body, so a correct
    // encoding leaves before the second statement runs. The control is the same model with
    // the escape checked only at the loop top, which pan refutes.
    Construct {
        name: "unless_midbody",
        pan: Some(0),
        source: "byte x = 0;\nactive proctype P() {\n    do :: x < 5 -> x = 1; x = x + 10 od unless (x == 1)\n}\nltl p0 { [] ( x <= 1 ) }\n",
    },
    Construct {
        name: "unless_midbody_toptop",
        pan: Some(1),
        source: "byte x = 0;\nactive proctype P() {\n    do :: (x < 5 && x != 1) -> x = 1; x = x + 10 od\n}\nltl p0 { [] ( x <= 1 ) }\n",
    },
    // The second half of the claim pair, which is what separates an acceptance condition from a
    // presence check: a check that reports a violation whenever a claim exists answers 1 for both.
    Construct {
        name: "never_unreachable",
        pan: Some(0),
        source: "/* The guard never holds, so the claim never completes: pan reports no error. */\nbyte x = 0;\n\nactive proctype P() {\n    x = 1\n}\n\nnever { (x == 7) }\n",
    },
    // The pending pair: spin's own automaton for `<> (got)`, embedded. Refused today (it carries
    // an `accept` label), so these rows assert nothing yet - and they are the ones that will say
    // whether accept labels are encoded at all.
    //
    // Worth recording what they actually test, because it is not what the name suggests: pan
    // reports "assertion violated" on the refuting model. The `accept_all` state sits behind an
    // infinite `do ... od` and is unreachable, so the verdict comes from the claim's own assert,
    // not from an accepting cycle - which is why these two do not settle accept-cycle support on
    // their own.
    Construct {
        name: "never_accept_holds",
        pan: Some(0),
        source: "/* The property holds: got is never set, so the claim's only move is its self-loop. */\nbool got = false;\nactive proctype P() { skip }\nnever  {    /* <> (got) */\nT0_init:\n\tdo\n\t:: atomic { ((got)) -> assert(!((got))) }\n\t:: (1) -> goto T0_init\n\tod;\naccept_all:\n\tskip\n}\n",
    },
    Construct {
        name: "never_accept_refutes",
        pan: Some(1),
        source: "/* The rendezvous handshake sets got, so the claim reaches its assert. */\nchan c = [0] of { bit };\nbool got = false;\nactive proctype Sender() { c ! 1 }\nactive proctype Receiver() { c ? got }\nnever  {    /* <> (got) */\nT0_init:\n\tdo\n\t:: atomic { ((got)) -> assert(!((got))) }\n\t:: (1) -> goto T0_init\n\tod;\naccept_all:\n\tskip\n}\n",
    },
    // The cycle pair: the accept state is reachable and spins, so the claim has an accepting cycle
    // even though it never completes and its asserts never fire. The oracle is `pan -a` - without
    // it pan reports 0 for both files, and someone checking with plain pan would conclude the pair
    // is vacuous.
    Construct {
        name: "never_cycle_accept",
        pan: Some(1),
        source: "/* The claim spins in an accept-labelled state forever: pan -a reports 1. */\nbyte x = 0;\nactive proctype P() { x = 1 }\nnever { accept_x: do :: (1) -> goto accept_x od }\n",
    },
    Construct {
        name: "never_cycle_unreachable",
        pan: Some(0),
        source: "/* The accept state is unreachable, so there is no accepting cycle: pan -a reports 0. */\nbyte x = 0;\nactive proctype P() { x = 1 }\nnever { do :: (0) -> skip od; accept_x: skip }\n",
    },
    Construct {
        name: "never_accept_unreachable_branch",
        pan: Some(0),
        source: "/* An accept-labelled state named only by a branch no reachable state can satisfy. pan -a 0:\n   the edge exists, so a translation that reads a branch body instead of its condition makes the\n   accept state reachable and refutes a claim pan accepts. */\nbyte x = 0;\nactive proctype P() { x = 1 }\nnever {\nT0:\n    do\n    :: (x == 7) -> goto accept_x\n    :: (1)     -> goto T0\n    od;\naccept_x:\n    skip\n}\n",
    },
    // The four boolean rows from the consumer's table: Promela has no boolean type, so `false` is
    // the integer 0 and `true` is 1, on initialisation and on assignment.
    Construct {
        name: "assert_not_bool_false",
        pan: Some(0),
        source: "bool flag = false;\nactive proctype P() { assert(!flag) }\n",
    },
    Construct {
        name: "assert_bool_equals_zero",
        pan: Some(0),
        source: "bool flag = false;\nactive proctype P() { assert(flag == 0) }\n",
    },
    Construct {
        name: "assign_bool_literal",
        pan: Some(1),
        source: "bool flag = false;\nactive proctype P() { flag = true }\nltl p0 { [] ( !flag ) }\n",
    },
    Construct {
        name: "assign_bool_to_bool",
        pan: Some(0),
        source: "bool a = true;\nbool b = false;\nactive proctype P() { b = a; assert(b == 1) }\n",
    },
    // A poll receive on a capacity-0 channel: pan does not pair it with a waiting sender, so the
    // receiver waits and the claim about the following statement is refuted. Written with the
    // eval-receive spelling because that is the construct, not the consumer's `??` row.
    Construct {
        name: "poll_receive_waits",
        pan: Some(1),
        source: "chan c = [0] of { bit };\nbit got = 0;\nactive proctype Sender() { c ! 1 }\nactive proctype Receiver() { c ? [1]; got = 1 }\nltl p0 { <> ( got ) }\n",
    },
    // The consumer's two random-receive rows, verbatim. Both need `pan -a`: without it pan reports 0
    // for both files, so a table built from plain pan records the same expectation twice and the pair
    // stops discriminating.
    Construct {
        name: "poll_rendezvous",
        pan: Some(0),
        source: "chan c = [0] of { bit };\nbyte got = 0;\nactive proctype Sender() { c ! 1 }\nactive proctype Receiver() { c ?? 1; got = 1 }\nltl p0 { <> ( got == 1 ) }\n",
    },
    Construct {
        name: "poll_no_sender",
        pan: Some(1),
        source: "chan c = [0] of { bit };\nbyte got = 0;\nactive proctype Receiver() { c ?? 1; got = 1 }\nltl p0 { <> ( got == 1 ) }\n",
    },
    // The idiomatic response property, boxed: refuted here and by pan, because `b` never becomes
    // 1 while `a` is 0 at a reachable state.
    Construct {
        name: "response_boxed",
        pan: Some(1),
        source: "byte a = 0;\nbyte b = 0;\nactive proctype P() { do :: true -> a = 1 od }\nltl p0 { [] ( a == 0 -> <> ( b == 1 ) ) }\n",
    },
    // The same property without the box: pan refutes it too (it evaluates the formula at the
    // initial state, where a == 0, so the consequent is owed), and this engine declines it by
    // name. A missing verdict on the idiomatic shape, recorded rather than guessed at - the two
    // readings differ on the model where the antecedent only becomes true later.
    Construct {
        name: "response_unboxed",
        pan: Some(1),
        source: "byte a = 0;\nbyte b = 0;\nactive proctype P() { do :: true -> a = 1 od }\nltl p0 { a == 0 -> <> ( b == 1 ) }\n",
    },
    // A proctype that declares parameters, started by `active`: pan binds every parameter to zero,
    // which is measurable rather than assumed. `run` still refuses, so a process whose arguments
    // come from a call has no verdict here yet.
    Construct {
        name: "active_proctype_parameter",
        pan: Some(0),
        source: "active proctype P(byte n) { assert(n == 0) }\n",
    },
    // The control: the same parameter is not 1, so the assertion is refuted.
    Construct {
        name: "active_proctype_parameter_refutes",
        pan: Some(1),
        source: "active proctype P(byte n) { assert(n == 1) }\n",
    },
    // The type half of the name/type/receiver family, which a declaration-level check cannot reach: a
    // type is a property of a *write*. 250 + 10 is 4 in a byte. The middle row is the discriminator -
    // an engine that truncates nothing fails it - so these guard anything that changes how a write is
    // emitted, and they pass because the truncation landed with the init locals in v0.5.29.
    Construct {
        name: "byte_write_truncates",
        pan: Some(0),
        source: "byte i = 250;\nactive proctype P() { i = i + 10; assert(i == 4) }\n",
    },
    Construct {
        name: "byte_write_does_not_overflow",
        pan: Some(1),
        source: "byte i = 250;\nactive proctype P() { i = i + 10; assert(i == 260) }\n",
    },
    Construct {
        name: "byte_write_bounded_claim",
        pan: Some(0),
        source: "byte i = 250;\nactive proctype P() { i = i + 10 }\nltl p0 { [] ( i <= 255 ) }\n",
    },
    // An increment as a guard: `:: i++`, which is how spin's own ex_1a.pml writes a loop body.
    // The guard-position heuristic read `i` as a condition and left `++` for the `od`.
    Construct {
        name: "increment_as_guard",
        pan: Some(0),
        source: "byte i = 0;\nactive proctype P() { do :: i++ -> skip od }\n",
    },
    // A declaration inside `init`, whose locals are laid out under the `init_` prefix the body
    // reads. Without it the body read a field that does not exist.
    Construct {
        name: "init_local",
        pan: Some(0),
        source: "byte g = 0;\ninit { byte i; i = 1; g = i }\nltl p0 { <> ( g == 1 ) }\n",
    },
    // A local initialised from `_pid`. `_pid` is the id of the process whose transition this is, so
    // in the state layout - which lays out every instance of a proctype one after another - it is
    // the instance index. Emitting it as the per-transition constant failed the whole initial
    // state, which is what ex_3a.pml hit.
    Construct {
        name: "pid_initialiser",
        pan: Some(0),
        source: "active [2] proctype p() { byte i = _pid; assert(i <= 1) }\n",
    },
    // The control: instance 1 has i == 1, so this assertion is refuted.
    Construct {
        name: "pid_initialiser_refutes",
        pan: Some(1),
        source: "active [2] proctype p() { byte i = _pid; assert(i == 0) }\n",
    },
    // `set_priority` moves a process's scheduling priority at run time. The filter here reads the
    // priorities declared on the proctype, so a model that moves them is scheduled by pan and not
    // by us. The emitter also passed the arguments without the state, so it crashed rather than
    // being wrong - either way, refused by name.
    Construct {
        name: "set_priority_call",
        pan: Some(0),
        source: "byte cnt;\nactive proctype P() priority 5 { set_priority(0, 8); cnt++ }\n",
    },
    // A multi-declarator inside a body: `byte o, i;`. The statement parser took one name and left
    // the comma, which took the whole proctype down with it - while the same declaration at top
    // level parsed, because it went through a different production.
    Construct {
        name: "multi_declarator_body",
        pan: Some(0),
        source: "active proctype P() { byte o, i; o = 1; assert(o == 1) }\n",
    },
    // An mtype-initialised local: the initialiser is evaluated in the state layout, where the
    // table is `state` and not `s`, and where mtype names had not been rewritten to numbers.
    // It produced `state.P_x = s.a`, failed the initial state, and the model explored nothing and
    // reported success.
    Construct {
        name: "mtype_initialised_local",
        pan: Some(1),
        source: "mtype = { a, b, c };\nactive proctype P() { mtype x = a; assert(x == 1) }\n",
    },
    // The control: `c` is 1, so the same assertion holds.
    Construct {
        name: "mtype_initialised_local_holds",
        pan: Some(0),
        source: "mtype = { a, b, c };\nactive proctype P() { mtype x = c; assert(x == 1) }\n",
    },
    // `provided` gated a proctype's every step and agreed with pan on the minimal shapes - and then
    // hid a deadlock on spin's own pathfinder.pml: pan 1, ours 0, from 6 states instead of 12.
    // Dropping the clause gave 25 states and agreement, so the gate is too strong in combination
    // with a blocking atomic region and the mechanism is not understood. Refused by name.
    Construct {
        name: "provided_blocking_atomic",
        pan: Some(1),
        source: "mtype = { free, busy };\nmtype mutex = free;\nbyte state = 0;\nactive proctype high() { do :: state = 1; atomic { mutex == free -> mutex = busy }; state = 2; atomic { state = 0; mutex = free } od }\nactive proctype low() provided (state == 0) { do :: atomic { mutex == free -> mutex = busy }; atomic { mutex = free } od }\n",
    },
    // mtype constants are numbered from 1 in *reverse* declaration order - measured with pan, four
    // assertions per list - so `free` is 2 in a two-name list. Numbering from 0 in declaration
    // order put every constant one below spin's and turned this assert inside out.
    Construct {
        name: "mtype_values",
        pan: Some(1),
        source: "mtype = { free, idle };\nbyte x = 0;\nactive proctype P() { x = free; assert(x == 1) }\n",
    },
    // A claim comparing against an mtype constant. The atom is parsed from a string long after the
    // AST walk that rewrites mtype names in bodies, so it compared against a field that does not
    // exist, read false, and refuted a claim pan accepts.
    Construct {
        name: "mtype_atom",
        pan: Some(0),
        source: "mtype = { free, idle };\nbyte x = 0;\nactive proctype P() { x = free }\nltl p0 { <> ( x == free ) }\n",
    },
    // `show mtype x = idle;` declares a display variable; skipping it would leave a model with
    // nothing to assign to.
    Construct {
        name: "show_declaration",
        pan: Some(0),
        source: "mtype = { free, idle };\nshow mtype h_state = idle;\nactive proctype P() { h_state = free }\nltl p0 { <> ( h_state == free ) }\n",
    },
    // A declaration ended by a newline instead of a semicolon, which spin accepts and its own
    // examples use.
    Construct {
        name: "decl_newline",
        pan: Some(0),
        source: "bool b[2]\nactive proctype P() { b[1] = 1 }\nltl p0 { <> ( b[1] == 1 ) }\n",
    },
    // A claim over a *local* variable: the state has no copy of it, so the atom is declined rather
    // than read as false. pan answers 0 for reasons this engine cannot reproduce - a verdict it
    // will not claim is the honest outcome, and the row keeps pan's number beside it.
    Construct {
        name: "claim_over_local",
        pan: Some(0),
        source: "active proctype P() { byte q; q = 1 }\nltl p0 { <> ( q == 1 ) }\n",
    },
    // An inline whose parameters are a channel and a value. The argument has to be substituted as
    // an *expression*: a bare name target stayed the parameter name, and the runtime rejected it
    // with "chan_send: expected integer values".
    Construct {
        name: "inline_send_argument",
        pan: Some(0),
        source: "inline snd(c, v) { c ! v }\nchan ch = [1] of { byte };\nbyte got = 0;\nactive proctype P() { snd(ch, 7); ch ? got }\nltl p0 { <> ( got == 7 ) }\n",
    },
    // A parameter used as an assignment target, which is the case a textual macro exists for.
    Construct {
        name: "inline_lvalue",
        pan: Some(0),
        source: "inline set(x) { x = 1 }\nbyte y = 0;\nactive proctype P() { set(y) }\nltl p0 { <> ( y == 1 ) }\n",
    },
    // A receive inside an inline used as a guard, so the call sits where a guard expression is
    // expected rather than where a statement is.
    Construct {
        name: "inline_guard",
        pan: Some(0),
        source: "inline take(c) { c ? 1 }\nchan ch = [1] of { byte };\nbyte done = 0;\nactive proctype P() { ch ! 1; if :: take(ch) -> done = 1 :: else -> skip fi }\nltl p0 { <> ( done == 1 ) }\n",
    },
    // `timeout` is true only when no other process can move. The guard emitted for it returns
    // false unconditionally, so the branch was dead and this model - which pan accepts, because
    // the only statement left is the timeout branch - was refuted here. Refused by name now:
    // a dead branch that decides the model is a wrong verdict, which is worse than a refusal.
    Construct {
        name: "timeout_branch",
        pan: Some(0),
        source: "byte x = 0;\nactive proctype P() { do :: timeout -> break od; x = 1 }\nltl p0 { <> ( x == 1 ) }\n",
    },
    // An inline call: the step machine emitted a call to a Lua global nothing defines, so this
    // failed at run time with `attempt to call a nil value`. Neither a verdict nor a refusal.
    Construct {
        name: "inline_call",
        pan: Some(0),
        source: "inline step(x) { y = x }\nbyte y = 0;\nactive proctype P() { step(1) }\nltl p0 { <> ( y == 1 ) }\n",
    },
    // A two-field message received into two variables. The message was always stored as a table
    // in the state; the receive handed back one value and the multi-variable shape was refused,
    // so a round trip through a structured message had no verdict at all.
    Construct {
        name: "multi_field_roundtrip",
        pan: Some(0),
        source: "chan c = [2] of { byte, byte };\nbyte a = 0;\nbyte b = 0;\nactive proctype P() { c ! 1, 7; c ? a, b; assert(b == 7) }\n",
    },
    // The control: the fields are bound in declaration order, so the first one is 1, not 7.
    Construct {
        name: "multi_field_order",
        pan: Some(1),
        source: "chan c = [2] of { byte, byte };\nbyte a = 0;\nbyte b = 0;\nactive proctype P() { c ! 1, 7; c ? a, b; assert(a == 7) }\n",
    },
    // A claim whose atom calls `len()` on a channel. The atom is compiled as its own chunk
    // against a bare state, so it resolved `chan_len` through aliases the generated model
    // defines for itself and found nil: the claim was declined while the same expression
    // inside a body worked. A rendezvous channel has length 0, so the implication is false.
    Construct {
        name: "len_atom_in_claim",
        pan: Some(1),
        source: "chan q;\nltl p0 { [] ( (len(q) < 2) -> (len(q) > 0) ) }\n",
    },
    // The control: with capacity 1 the same predicate holds however much is queued.
    Construct {
        name: "len_atom_holds",
        pan: Some(0),
        source: "chan q = [1] of { byte };\nbyte d = 0;\nactive proctype P() { q ! 1; d = 1 }\nltl p0 { [] ( len(q) <= 1 ) }\n",
    },
    // A handler whose guard only becomes true *after* the body's second step, so the escape
    // boundary is tested at a later step than the first. pan refutes: the handler preempts
    // mid-body and sets y = 9. A counter off by one at this boundary would miss it and
    // answer 0.
    Construct {
        name: "unless_straddle_mid",
        pan: Some(1),
        source: "byte x = 0;\nbyte y = 0;\nactive proctype P() { { x = 1; x = 2; x = 3 } unless { (x == 2); y = 9 } }\nltl p0 { [] ( y <= 2 ) }\n",
    },
    // The handler's guard becomes true only at the body's *last* step, and pan accepts: the
    // escape exists while the body is mid-flight, so once the body has completed there is
    // nothing left to preempt, and y stays 0. An engine that let the handler run after the
    // body finished would answer 1 here.
    Construct {
        name: "unless_straddle_last",
        pan: Some(0),
        source: "byte x = 0;\nbyte y = 0;\nactive proctype P() { { x = 1; x = 2; x = 3 } unless { (x == 3); y = 9 } }\nltl p0 { [] ( y <= 2 ) }\n",
    },
    // A handler of two statements: the second statement sets y = 9, which refutes the claim. An
    // engine that ran only the first statement would answer 0 against pan's 1.
    Construct {
        name: "unless_multi_handler",
        pan: Some(1),
        source: "byte x = 0;\nbyte y = 0;\nactive proctype P() { { x = 1; y = 2 } unless { (x == 1); y = 9 } }\nltl p0 { [] ( y <= 2 ) }\n",
    },
    // `?` tests the head and `??` searches the queue: with 1 at the head, `c ?? 2` takes the 2 and
    // the following receive still finds a message, where `c ? 2` would block. Two receives after
    // the random one is what makes the difference visible - one receive cannot tell a search
    // from a head test, because either way a message is left behind.
    Construct {
        name: "buf_random_search",
        pan: Some(1),
        source: "chan c = [2] of { byte };\nbyte y = 0;\nbyte z = 0;\nbyte done = 0;\nactive proctype P() { c ! 1; c ! 2; c ?? 2; c ? y; c ? z; done = 1 }\nltl p0 { <> ( done ) }\n",
    },
    // The search starts at the head: with the queue [1, 2, 1] and `c ?? 1`, pan removes the
    // *first* 1, leaving [2, 1], so the next receive binds 2. Removing the last match instead
    // leaves [1, 2] and the assertion fails - measured, this is pan 0.
    Construct {
        name: "buf_random_first_match",
        pan: Some(0),
        source: "chan c = [3] of { byte };\nbyte a = 0;\nbyte b = 0;\nactive proctype P() { c ! 1; c ! 2; c ! 1; c ?? 1; c ? a; c ? b; assert(a == 2) }\n",
    },
    // A plain receive naming a value does not search: the head is 1, so it blocks and `done` is
    // never set.
    Construct {
        name: "buf_match_head_only",
        pan: Some(1),
        source: "chan c = [2] of { byte };\nbyte y = 0;\nbyte done = 0;\nactive proctype P() { c ! 1; c ! 2; c ? 2; done = 1 }\nltl p0 { <> ( done ) }\n",
    },
    // The control: with the matching message at the head the same receive is enabled.
    Construct {
        name: "buf_match_head_ok",
        pan: Some(0),
        source: "chan c = [2] of { byte };\nbyte y = 0;\nbyte done = 0;\nactive proctype P() { c ! 2; c ! 1; c ? 2; done = 1 }\nltl p0 { <> ( done ) }\n",
    },
    // A two-field message received into one variable. pan rejects the arity at run time and
    // reports 1; binding the first field and dropping the second decided 0 here. Refused by
    // name, because matching pan's number would mean encoding pan's model error as a verdict.
    Construct {
        name: "buf_arity_mismatch",
        pan: Some(1),
        source: "chan c = [2] of { byte, byte };\nbyte a = 0;\nbyte done = 0;\nactive proctype P() { c ! 1, 7; c ! 2, 8; c ?? 2, 8; c ? a; done = 1 }\nltl p0 { <> ( done ) }\n",
    },
    // A model that blocks at once, under a claim. pan's summary says "invalid end states -
    // (disabled by never claim)", so its answer is the claim's, not the model's: 0 here and 1 on
    // the [] ( x == 1 ) variant below. Reporting a model-level error refutes a plan whose
    // constraints all hold, which is what a consumer's guard saw.
    Construct {
        name: "deadlock_under_claim",
        pan: Some(0),
        source: "chan c = [0] of { bit };\nbyte x = 0;\n\nactive proctype P() {\n    c ? x\n}\n\nltl p0 { [] ( x == 0 ) }\n",
    },
    // x is never assigned at all, so the claim that it is 1 is refuted on the first state, and
    // the model also blocks - the count is 1 for the claim, and the deadlock must not make it 2.
    Construct {
        name: "deadlock_claim_refuted",
        pan: Some(1),
        source: "chan c = [0] of { bit };\nbyte x = 0;\n\nactive proctype P() {\n    c ? x\n}\n\nltl p0 { [] ( x == 1 ) }\n",
    },
    // No claim: now the invalid end state *is* the error, and pan reports one.
    Construct {
        name: "deadlock_no_claim",
        pan: Some(1),
        source: "chan c = [0] of { bit };\nbyte x = 0;\n\nactive proctype P() {\n    c ? x\n}\n",
    },
    // A comparison atom in a *liveness* claim. The safety path decided this atom from the state;
    // the liveness path asked the initial state for a field named "x == 1" and declined.
    Construct {
        name: "liveness_comparison_atom",
        pan: Some(0),
        source: "byte x = 0;\n\nactive proctype P() {\n    x = 1\n}\n\nltl p0 { <> ( x == 1 ) }\n",
    },
    // The same shape where the claim is false forever: pan finds the accepting cycle in the one
    // reachable state, so the answer is 1 - and it is the claim's 1, not a deadlock's.
    Construct {
        name: "stuck_liveness_refuted",
        pan: Some(1),
        source: "chan c = [0] of { bit };\nbyte x = 0;\n\nactive proctype P() {\n    c ? x\n}\n\nltl p0 { <> ( x == 1 ) }\n",
    },
];

/// What this engine answered for one construct.
pub fn outcome(c: &Construct) -> (Ours, bool) {
    match crate::runtime::verify(c.source) {
        Ok(r) if r.unverified.is_empty() => (Ours::Verdict(r.errors), false),
        Ok(r) => {
            // Refuted *and* undecided is the one combination the result type exists to prevent.
            let both = r.violations.iter().any(|v| {
                r.unverified
                    .iter()
                    .any(|u| u.property_name == v.property_name)
            });
            (Ours::Decline, both)
        }
        Err(_) => (Ours::Refuse, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_construct_is_decided_against_pan() {
        let mut decided = 0;
        let mut undecided = 0;
        for c in CONSTRUCTS {
            let Some(pan) = c.pan else {
                continue;
            };
            let (ours, refuted_while_declining) = outcome(c);
            assert!(
                !refuted_while_declining,
                "{}: a claim must not be refuted and reported undecided at once",
                c.name
            );
            match ours {
                Ours::Verdict(n) => {
                    decided += 1;
                    assert_eq!(
                        n, pan,
                        "{}: decided the model and disagreed with pan (ours {} vs pan {})",
                        c.name, n, pan
                    );
                }
                Ours::Decline | Ours::Refuse => undecided += 1,
            }
        }
        // If every construct is refused the test above proves nothing, and a change that silently
        // stopped refusing would pass it. Some must be decided.
        assert!(
            decided > 0,
            "the table must decide at least one construct, or it is not measuring verdicts"
        );
        assert_eq!(decided + undecided, CONSTRUCTS.len());
        eprintln!(
            "CONSTRUCT-COUNT rows={} decided={} undecided={}",
            CONSTRUCTS.len(),
            decided,
            undecided
        );
    }

    #[test]
    fn a_refused_construct_is_refused_by_name() {
        // The README claims these are refused by name rather than by a parse error carrying an
        // offset, which a caller cannot tell apart from a malformed model of their own.
        // `unless` and `priority` used to be in this list; both moved to being decided when their
        // encodings landed, which is this table doing what it was built for.
        let c = CONSTRUCTS
            .iter()
            .find(|c| c.name == "run")
            .expect("run must be in the table");
        let err = crate::runtime::verify(c.source)
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default();
        assert!(
            err.contains("run is not supported"),
            "run must be refused by name, got: {err}"
        );
    }
}
