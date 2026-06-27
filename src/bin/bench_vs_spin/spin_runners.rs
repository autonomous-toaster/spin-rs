// ─── Spin Runners ────────────────────────────────────────────────────

use spin_rs::{CheckResult, CheckerBuilder, LuaModel, SearchMode, StorageMode};
use std::process::{Command, Stdio};

pub(crate) fn get_spin_version() -> Option<String> {
    let o = Command::new("spin").arg("-V").output().ok()?;
    if o.status.success() {
        Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
    } else {
        None
    }
}

#[derive(Debug, Clone)]
pub struct SpinOutput {
    pub states: usize,
    pub transitions: usize,
    pub errors: usize,
    pub depth: usize,
}

pub(crate) fn run_spin(source: &str, bfs: bool, por: bool) -> Option<SpinOutput> {
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

pub(crate) fn spin_compile(source: &str) -> Result<(), String> {
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

pub(crate) fn spin_run_compiled() -> Option<SpinOutput> {
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

pub(crate) fn parse_spin(stdout: &str) -> Option<SpinOutput> {
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

pub fn run_spin_rs(
    source: &str,
    storage: StorageMode,
    search: SearchMode,
    por: bool,
) -> CheckResult {
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
