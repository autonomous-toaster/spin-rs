//! Benchmark suite: spin-rs vs Spin 6.5.x.
//!
//! Run with: cargo run --release --bin bench_vs_spin
//!
//! Three phases:
//!   1. Correctness equivalence — compare outputs (states, errors, violations)
//!   2. Local performance — profile spin-rs time breakdown
//!   3. Global comparison — wall-clock vs Spin (full pipeline + verify-only)

#![allow(dead_code)]

use std::process::{Command, Stdio};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use spin_rs::{CheckResult, CheckerBuilder, LuaModel, SearchMode, StorageMode};

// ─── Model Corpus ────────────────────────────────────────────────────

const PLAN_5TASKS_3LTLS: &str = r#"
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

const PLAN_20TASKS_10LTLS: &str = r#"
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

const ASSERTION_SAFETY: &str = r#"
active proctype Main() { byte x = 0; x = 1; assert(x == 1); }
"#;

const MULTI_PROCESS: &str = r#"
byte counter = 0;
active proctype A() {
    do :: counter < 10 -> counter = counter + 1 :: counter >= 10 -> break od
}
active proctype B() {
    do :: counter < 10 -> counter = counter + 10 :: counter >= 10 -> break od
}
"#;

const DEADLOCK_CIRCULAR: &str = r#"
chan ch1 = [0] of { byte }; chan ch2 = [0] of { byte };
active proctype P() { ch1 ! 1; ch2 ? 0; }
active proctype Q() { ch2 ! 1; ch1 ? 0; }
"#;

const LTL_VIOLATION: &str = r#"
byte x = 0;
active proctype P() { do :: x = 0 :: x = 1 od }
ltl p0 { [](x == 0) }
"#;

const PETERSON_N2: &str = r#"
byte turn; byte flag[2];
active [2] proctype user() {
    do :: flag[_pid] = 1; turn = _pid;
       (flag[1-_pid] == 0 || turn != _pid);
       flag[_pid] = 0;
    od
}
"#;

const PETERSON_N3: &str = r#"
byte turn; byte flag[3];
active [3] proctype user() {
    do :: flag[_pid] = 1; turn = _pid;
       (flag[1-_pid] == 0 || turn != _pid);
       flag[_pid] = 0;
    od
}
"#;

const DINING_N4: &str = r#"
byte fork[4];
inline pickup(i) { atomic { (fork[i] == 0); fork[i] = 1 } }
inline putdown(i) { fork[i] = 0 }
active [4] proctype philosopher() {
    do :: pickup(_pid); pickup((_pid + 1) % 4);
       putdown(_pid); putdown((_pid + 1) % 4) od
}
"#;

const TOKEN_RING_N5: &str = r#"
chan tok[5];
init { byte i; for (i in 0 .. 4) { tok[i] = [1] of { byte } }; tok[0] ! 1 }
active [5] proctype node() {
    byte msg;
    do :: tok[_pid] ? msg ->
         if :: msg == 1 -> tok[(_pid + 1) % 5] ! msg :: else -> skip fi
    od
}
"#;

const STATE_EXPLOSION: &str = r#"
byte a, b, c, d, e;
active proctype Counter() {
    do :: a < 1 -> a = a + 1 :: b < 1 -> b = b + 1 :: c < 1 -> c = c + 1
       :: d < 1 -> d = d + 1 :: e < 1 -> e = e + 1
       :: (a == 1 && b == 1 && c == 1 && d == 1 && e == 1) -> break
    od
}
"#;

const SINGLE_LOOP: &str = r#"
active proctype P() {
    byte x = 0;
    do :: x < 100 -> x = x + 1 :: x >= 100 -> break od
}
"#;

struct ModelDef {
    name: &'static str,
    source: &'static str,
    expected_errors: usize,
}

const ALL_MODELS: &[ModelDef] = &[
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

// ─── Main ────────────────────────────────────────────────────────────

fn main() {
    println!("═══ spin-rs Benchmark Suite ═══");
    println!("Models: {}", ALL_MODELS.len());
    println!();

    let spin_version = get_spin_version();
    match &spin_version {
        Some(v) => println!("Spin:   {v}"),
        None => eprintln!("WARN: Spin not on PATH — Phases 1 and 3 skip Spin"),
    }
    println!();

    let (p1_results, p1_report) = phase1_correctness(spin_version.is_some());
    println!();
    let (p2_results, p2_report) = phase2_local_perf();
    println!();
    let (p3_results, p3_report) = phase3_global(spin_version.is_some());

    // Write JSON report (T3.7)
    let json = serde_json::json!({
        "timestamp": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        "spin_version": spin_version,
        "models": ALL_MODELS.len(),
        "phase1": { "results": p1_results, "report": p1_report },
        "phase2": { "results": p2_results, "report": p2_report },
        "phase3": { "results": p3_results, "report": p3_report },
    });
    let dir = std::path::Path::new("target/bench-results");
    let _ = std::fs::create_dir_all(dir);
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let path = dir.join(format!("{ts}.json"));
    if let Ok(s) = serde_json::to_string_pretty(&json) {
        let _ = std::fs::write(&path, &s);
        println!("\nJSON results: {}", path.display());
    }

    println!("\n═══ Done ═══");
}

// ─── Phase 1: Correctness Equivalence ────────────────────────────────

fn phase1_correctness(spin_available: bool) -> (Vec<serde_json::Value>, String) {
    println!("─── Phase 1: Correctness Equivalence ───");
    let mut results = Vec::new();
    let mut pass = 0u32;
    let mut warn = 0u32;
    let mut fail = 0u32;

    for model in ALL_MODELS {
        for &(config, search, por, tol) in &[
            ("Exact+DFS+noPOR", SearchMode::DepthFirst, false, 0),
            ("Exact+DFS+POR  ", SearchMode::DepthFirst, true, 1),
            ("Exact+BFS+noPOR", SearchMode::BreadthFirst, false, 0),
        ] {
            let sr = run_spin_rs(model.source, StorageMode::Exact, search, por);
            let sp = if spin_available {
                run_spin(model.source, search == SearchMode::BreadthFirst, por)
            } else {
                None
            };
            let status = check_correctness(&sr, sp.as_ref(), model.expected_errors, tol);
            let icon = match &status {
                Ok(()) => "✓",
                Err(s) if s.starts_with("state") => "⚠",
                _ => "✗",
            };
            let detail = match &status {
                Ok(()) => String::new(),
                Err(e) => format!(": {e}"),
            };
            println!("  {icon} {:25} {config}{detail}", model.name);
            match &status {
                Ok(()) => pass += 1,
                Err(s) if s.starts_with("state") && tol > 0 => warn += 1,
                _ => fail += 1,
            }
            results.push(serde_json::json!({
                "model": model.name, "config": config.trim(),
                "spin_rs_states": sr.states_explored, "spin_rs_errors": sr.errors,
                "spin_states": sp.as_ref().map(|s| s.states),
                "spin_errors": sp.as_ref().map(|s| s.errors),
                "status": if status.is_ok() { "pass" } else if let Err(s) = &status { if s.starts_with("state") { "warn" } else { "fail" } } else { "pass" },
            }));
        }
    }
    let report = format!("{pass} pass, {warn} warn, {fail} fail");
    println!("\n  Result: {report}");
    (results, report)
}

fn check_correctness(
    sr: &CheckResult,
    sp: Option<&SpinOutput>,
    expected: usize,
    tol: usize,
) -> Result<(), String> {
    if sr.errors != expected {
        return Err(format!(
            "spin-rs errors {} != expected {}",
            sr.errors, expected
        ));
    }
    if let Some(sp) = sp {
        if sr.errors != sp.errors {
            return Err(format!("errors spin-rs={} spin={}", sr.errors, sp.errors));
        }
        if tol == 0 && sr.states_explored != sp.states {
            return Err(format!(
                "states spin-rs={} spin={}",
                sr.states_explored, sp.states
            ));
        }
        if tol > 0 {
            let s = sp.states.max(1);
            let d = if sr.states_explored > s {
                (sr.states_explored - s) * 100 / s
            } else {
                (s - sr.states_explored) * 100 / s
            };
            if d > tol {
                return Err(format!("states differ by {d}% (tol {tol}%)"));
            }
        }
    }
    Ok(())
}

// ─── Phase 2: Local Performance ──────────────────────────────────────

fn phase2_local_perf() -> (Vec<serde_json::Value>, String) {
    println!("─── Phase 2: Local Performance ───");
    let mut results = Vec::new();

    println!(
        "{:<25} {:>8} {:>8} {:>8} {:>8} {:>6} {:>10}",
        "Model", "Parse", "CG", "Boot", "Verify", "States", "T/s"
    );
    println!("{}", "-".repeat(85));

    let models = ["plan_5tasks_3ltls", "single_loop", "state_explosion"];
    for model in ALL_MODELS.iter().filter(|m| models.contains(&m.name)) {
        let start = Instant::now();
        let ast = spin_rs::parse(model.source).unwrap();
        let p = start.elapsed().as_micros();
        let start = Instant::now();
        let _lua = spin_rs::generate_lua(&ast);
        let cg = start.elapsed().as_micros();
        let start = Instant::now();
        let r = spin_rs::verify(model.source).unwrap();
        let v = start.elapsed().as_micros();
        let start = Instant::now();
        let _lm = spin_rs::create_model(model.source).unwrap();
        let b = start.elapsed().as_micros();
        let tps = if v > 0 {
            r.transitions as f64 / (v as f64 / 1_000_000.0)
        } else {
            0.0
        };
        println!(
            "{:<25} {:>8} {:>8} {:>8} {:>8} {:>6} {:>10.0}",
            model.name, p, cg, b, v, r.states_explored, tps
        );
        results.push(serde_json::json!({
            "model": model.name, "parse_us": p, "codegen_us": cg,
            "boot_us": b, "verify_us": v, "states": r.states_explored,
            "transitions_per_sec": tps,
        }));
    }

    #[cfg(feature = "bench")]
    if let Ok(lua) = mlua::Lua::new() {
        if let Ok(f) = lua.load("return function() end").eval::<mlua::Function>() {
            let n = 1_000_000u64;
            let start = Instant::now();
            for _ in 0..n {
                let _: () = f.call(()).ok();
            }
            let ns = start.elapsed().as_nanos() / n as u128;
            println!("\n  Lua↔Rust FFI roundtrip: {ns} ns/call");
            results.push(serde_json::json!({ "ffi_roundtrip_ns": ns }));
        }
    }
    #[cfg(not(feature = "bench"))]
    println!("\n  Lua↔Rust FFI roundtrip: use --features bench for detailed measurement");

    let n = results.len();
    (results, format!("{n} models profiled"))
}

// ─── Phase 3: Global Comparison ──────────────────────────────────────

fn phase3_global(spin_available: bool) -> (Vec<serde_json::Value>, String) {
    println!("─── Phase 3: Global Comparison ───");
    let mut results = Vec::new();
    println!(
        "{:<25} {:>10} {:>10} {:>10} {:>10} {:>10}",
        "Model", "sr(s)", "Spin(s)", "Speedup", "States", "St/s"
    );
    println!("{}", "-".repeat(90));

    for model in ALL_MODELS {
        // spin-rs: 5 runs, median
        let mut t = Vec::new();
        for _ in 0..5 {
            let s = Instant::now();
            let _ = spin_rs::verify(model.source);
            t.push(s.elapsed().as_secs_f64());
        }
        t.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let sr_med = t[2];
        let r = spin_rs::verify(model.source).unwrap();
        let states = r.states_explored;

        // Spin: compile once, run 5 times
        let sp_med = if spin_available {
            let _ = spin_compile(model.source);
            let mut t2 = Vec::new();
            for _ in 0..5 {
                let s = Instant::now();
                spin_run_compiled();
                t2.push(s.elapsed().as_secs_f64());
            }
            t2.sort_by(|a, b| a.partial_cmp(b).unwrap());
            Some(t2[2])
        } else {
            None
        };

        let spd = match sp_med {
            Some(s) if s > 0.0 => format!("{:.1}x", s / sr_med),
            _ => "N/A".into(),
        };
        let sts = if sr_med > 0.0 {
            format!("{:.0}", states as f64 / sr_med)
        } else {
            "N/A".into()
        };
        let sps = match sp_med {
            Some(s) => format!("{s:.4}"),
            None => "N/A".into(),
        };

        println!(
            "{:<25} {:>10.4} {:>10} {:>10} {:>10} {:>10}",
            model.name, sr_med, sps, spd, states, sts
        );
        results.push(serde_json::json!({
            "model": model.name, "spin_rs_time_s": sr_med,
            "spin_time_s": sp_med, "speedup": sp_med.map(|s| if sr_med > 0.0 { s / sr_med } else { 0.0 }),
            "states": states,
        }));
    }
    let n = results.len();
    (results, format!("{n} models compared"))
}

// ─── Spin Runners ────────────────────────────────────────────────────

fn get_spin_version() -> Option<String> {
    let o = Command::new("spin").arg("-V").output().ok()?;
    if o.status.success() {
        Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
    } else {
        None
    }
}

#[derive(Debug, Clone)]
struct SpinOutput {
    states: usize,
    transitions: usize,
    errors: usize,
    depth: usize,
}

fn run_spin(source: &str, bfs: bool, por: bool) -> Option<SpinOutput> {
    let tmp = std::env::temp_dir().join("spin_bench");
    let _ = std::fs::create_dir_all(&tmp);
    std::fs::write(tmp.join("model.pml"), source).ok()?;
    Command::new("spin")
        .arg("-a")
        .arg(tmp.join("model.pml"))
        .current_dir(&tmp)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let mut g = Command::new("gcc");
    g.args(["-O2", "-o", "pan", "pan.c"])
        .current_dir(&tmp)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if !por {
        g.arg("-DNOREDUCE");
    }
    if bfs {
        g.arg("-DS_BFS");
    }
    g.output().ok().filter(|o| o.status.success())?;
    let mut p = Command::new("./pan");
    p.current_dir(&tmp)
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    p.arg("-n"); // -n for no listing of unreached states
    let o = p.output().ok()?;
    parse_spin(&String::from_utf8_lossy(&o.stdout))
}

fn spin_compile(source: &str) -> Result<(), String> {
    let tmp = std::env::temp_dir().join("spin_bench");
    let _ = std::fs::create_dir_all(&tmp);
    std::fs::write(tmp.join("model.pml"), source).map_err(|e| e.to_string())?;
    Command::new("spin")
        .arg("-a")
        .arg(tmp.join("model.pml"))
        .current_dir(&tmp)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| e.to_string())
        .and_then(|o| {
            if o.status.success() {
                Ok(())
            } else {
                Err("spin -a failed".into())
            }
        })?;
    Command::new("gcc")
        .args(["-O2", "-o", "pan", "pan.c"])
        .current_dir(&tmp)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| e.to_string())
        .and_then(|o| {
            if o.status.success() {
                Ok(())
            } else {
                Err("gcc failed".into())
            }
        })
}

fn spin_run_compiled() -> Option<SpinOutput> {
    let tmp = std::env::temp_dir().join("spin_bench");
    let o = Command::new("./pan")
        .arg("-n")
        .current_dir(&tmp)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    parse_spin(&String::from_utf8_lossy(&o.stdout))
}

fn parse_spin(stdout: &str) -> Option<SpinOutput> {
    let mut states = 0;
    let mut transitions = 0;
    let mut errors = 0;
    for line in stdout.lines() {
        let t = line.trim();
        if t.starts_with("State-vector")
            && t.contains("errors:")
            && let Some(e) = t
                .split("errors:")
                .nth(1)
                .and_then(|s| s.split_whitespace().next())
                .and_then(|s| s.parse().ok())
        {
            errors = e;
        }
        if t.contains("states, stored")
            && let Some(n) = t.split_whitespace().next().and_then(|s| s.parse().ok())
        {
            states = n;
        }
        if t.contains("transitions")
            && !t.starts_with("#")
            && let Some(n) = t.split_whitespace().next().and_then(|s| s.parse().ok())
        {
            transitions = n;
        }
    }
    Some(SpinOutput {
        states,
        transitions,
        errors,
        depth: 0,
    })
}

// ─── spin-rs Runners ─────────────────────────────────────────────────

fn run_spin_rs(source: &str, storage: StorageMode, search: SearchMode, por: bool) -> CheckResult {
    CheckerBuilder::new()
        .model(LuaModel::from_source(source).unwrap())
        .storage_mode(storage)
        .search_mode(search)
        .por_enabled(por)
        .max_states(1_000_000)
        .max_depth(100_000)
        .build()
        .check()
}
