//! Benchmark suite: spin-rs vs Spin 6.5.x.
//!
//! Run with: cargo run --release --bin bench_vs_spin
//!
//! Three phases:
//!   1. Correctness equivalence — compare outputs (states, errors, violations)
//!   2. Local performance — profile spin-rs time breakdown
//!   3. Global comparison — wall-clock vs Spin (full pipeline + verify-only)

#![allow(dead_code)]

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use spin_rs::{CheckResult, SearchMode, StorageMode};

mod models;
mod spin_runners;
use models::*;
use spin_runners::*;

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
