// ─── Model Corpus ────────────────────────────────────────────────────

pub const PLAN_5TASKS_3LTLS: &str = r#"
bool t1_1, t1_2, t1_3, t2_1, t2_2;
active proctype task_t1_1() { do :: (1) -> t1_1 = 1; break od }
active proctype task_t1_2() { do :: (1) -> t1_2 = 1; break od }
active proctype task_t1_3() { do :: (1) -> t1_3 = 1; break od }
active proctype task_t2_1() { do :: (t1_1 && t1_2 && t1_3) -> t2_1 = 1; break od }
active proctype task_t2_2() { do :: (t2_1) -> t2_2 = 1; break od }
ltl p0 { [] (t2_1 -> (t1_1 && t1_2 && t1_3)) }
ltl p1 { [] (t2_2 -> t2_1) }
ltl p2 { [] ( !(t1_1 && t1_2 && t1_3 && !t2_1 && !t2_2) ) }
"#;

pub const PLAN_20TASKS_10LTLS: &str = r#"
bool t1_1, t1_2, t1_3, t1_4, t1_5;
bool t2_1, t2_2, t2_3, t2_4, t2_5;
bool t3_1, t3_2, t3_3, t3_4, t3_5;
bool t4_1, t4_2, t4_3, t4_4, t4_5;
active proctype task_t1_1() { do :: (1) -> t1_1 = 1; break od }
active proctype task_t1_2() { do :: (1) -> t1_2 = 1; break od }
active proctype task_t1_3() { do :: (1) -> t1_3 = 1; break od }
active proctype task_t1_4() { do :: (1) -> t1_4 = 1; break od }
active proctype task_t1_5() { do :: (1) -> t1_5 = 1; break od }
active proctype task_t2_1() { do :: (t1_1 && t1_2) -> t2_1 = 1; break od }
active proctype task_t2_2() { do :: (t1_3 && t1_4) -> t2_2 = 1; break od }
active proctype task_t2_3() { do :: (t1_5) -> t2_3 = 1; break od }
active proctype task_t2_4() { do :: (t2_1) -> t2_4 = 1; break od }
active proctype task_t2_5() { do :: (t2_2) -> t2_5 = 1; break od }
active proctype task_t3_1() { do :: (t2_3) -> t3_1 = 1; break od }
active proctype task_t3_2() { do :: (t2_4 && t2_5) -> t3_2 = 1; break od }
active proctype task_t3_3() { do :: (t3_1) -> t3_3 = 1; break od }
active proctype task_t3_4() { do :: (t3_2) -> t3_4 = 1; break od }
active proctype task_t3_5() { do :: (t3_3) -> t3_5 = 1; break od }
active proctype task_t4_1() { do :: (t3_4 && t3_5) -> t4_1 = 1; break od }
active proctype task_t4_2() { do :: (t4_1) -> t4_2 = 1; break od }
active proctype task_t4_3() { do :: (t4_2) -> t4_3 = 1; break od }
active proctype task_t4_4() { do :: (t4_3) -> t4_4 = 1; break od }
active proctype task_t4_5() { do :: (t4_4) -> t4_5 = 1; break od }
ltl p0 { [] (t2_1 -> (t1_1 && t1_2)) }
ltl p1 { [] (t2_2 -> (t1_3 && t1_4)) }
ltl p2 { [] (t3_2 -> (t2_4 && t2_5)) }
ltl p3 { [] (t4_1 -> (t3_4 && t3_5)) }
ltl p4 { [] (t4_5 -> t4_4) }
ltl p5 { [] (t4_4 -> t4_3) }
ltl p6 { [] (t4_3 -> t4_2) }
ltl p7 { [] (t4_2 -> t4_1) }
ltl p8 { [] (t3_5 -> t3_3) }
ltl p9 { [] (t3_3 -> t3_1) }
"#;

pub const ASSERTION_SAFETY: &str = r#"
active proctype Main() { byte x = 0; x = 1; assert(x == 1); }
"#;

pub const MULTI_PROCESS: &str = r#"
byte counter = 0;
active proctype A() {
    do :: counter < 10 -> counter = counter + 1 :: counter >= 10 -> break od
}
active proctype B() {
    do :: counter < 10 -> counter = counter + 10 :: counter >= 10 -> break od
}
"#;

pub const DEADLOCK_CIRCULAR: &str = r#"
chan ch1 = [0] of { byte }; chan ch2 = [0] of { byte };
active proctype P() { ch1 ! 1; ch2 ? 0; }
active proctype Q() { ch2 ! 1; ch1 ? 0; }
"#;

pub const LTL_VIOLATION: &str = r#"
byte x = 0;
active proctype P() { do :: x = 0 :: x = 1 od }
ltl p0 { [](x == 0) }
"#;

pub const PETERSON_N2: &str = r#"
byte turn; byte flag[2];
active [2] proctype user() {
    do :: flag[_pid] = 1; turn = _pid;
       (flag[1-_pid] == 0 || turn != _pid);
       flag[_pid] = 0;
    od
}
"#;

pub const PETERSON_N3: &str = r#"
byte turn; byte flag[3];
active [3] proctype user() {
    do :: flag[_pid] = 1; turn = _pid;
       (flag[1-_pid] == 0 || turn != _pid);
       flag[_pid] = 0;
    od
}
"#;

pub const DINING_N4: &str = r#"
byte fork[4];
inline pickup(i) { atomic { (fork[i] == 0); fork[i] = 1 } }
inline putdown(i) { fork[i] = 0 }
active [4] proctype philosopher() {
    do :: pickup(_pid); pickup((_pid + 1) % 4);
       putdown(_pid); putdown((_pid + 1) % 4) od
}
"#;

pub const TOKEN_RING_N5: &str = r#"
chan tok[5];
init { byte i; for (i in 0 .. 4) { tok[i] = [1] of { byte } }; tok[0] ! 1 }
active [5] proctype node() {
    byte msg;
    do :: tok[_pid] ? msg ->
         if :: msg == 1 -> tok[(_pid + 1) % 5] ! msg :: else -> skip fi
    od
}
"#;

pub const STATE_EXPLOSION: &str = r#"
byte a, b, c, d, e;
active proctype Counter() {
    do :: a < 1 -> a = a + 1 :: b < 1 -> b = b + 1 :: c < 1 -> c = c + 1
       :: d < 1 -> d = d + 1 :: e < 1 -> e = e + 1
       :: (a == 1 && b == 1 && c == 1 && d == 1 && e == 1) -> break
    od
}
"#;

pub const SINGLE_LOOP: &str = r#"
active proctype P() {
    byte x = 0;
    do :: x < 100 -> x = x + 1 :: x >= 100 -> break od
}
"#;

/// Benchmark model definition.
pub struct ModelDef {
    pub name: &'static str,
    pub source: &'static str,
    pub expected_errors: usize,
}

pub const ALL_MODELS: &[ModelDef] = &[
    ModelDef {
        name: "plan_5tasks_3ltls",
        source: PLAN_5TASKS_3LTLS,
        expected_errors: 0,
    },
    ModelDef {
        name: "plan_20tasks_10ltls",
        source: PLAN_20TASKS_10LTLS,
        expected_errors: 0,
    },
    ModelDef {
        name: "assertion_safety",
        source: ASSERTION_SAFETY,
        expected_errors: 0,
    },
    ModelDef {
        name: "multi_process",
        source: MULTI_PROCESS,
        expected_errors: 0,
    },
    ModelDef {
        name: "deadlock_circular",
        source: DEADLOCK_CIRCULAR,
        expected_errors: 1,
    },
    ModelDef {
        name: "ltl_violation",
        source: LTL_VIOLATION,
        expected_errors: 1,
    },
    ModelDef {
        name: "peterson_n2",
        source: PETERSON_N2,
        expected_errors: 0,
    },
    ModelDef {
        name: "peterson_n3",
        source: PETERSON_N3,
        expected_errors: 0,
    },
    ModelDef {
        name: "dining_n4",
        source: DINING_N4,
        expected_errors: 0,
    },
    ModelDef {
        name: "token_ring_n5",
        source: TOKEN_RING_N5,
        expected_errors: 0,
    },
    ModelDef {
        name: "state_explosion",
        source: STATE_EXPLOSION,
        expected_errors: 0,
    },
    ModelDef {
        name: "single_loop",
        source: SINGLE_LOOP,
        expected_errors: 0,
    },
];
