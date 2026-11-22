//! Differential parity gate against pan.
//!
//! Two modes:
//!
//! - `--baseline` records what the current binary does against pan and writes the report into
//!   the change directory.
//! - `--gate` fails when the corpus contains a wrong verdict that is not already listed in
//!   the recorded baseline, and warns when a recorded one starts passing, so the list cannot rot.
//! - `--dir <path>` runs a directory of `.pml` files instead of the generated corpus, which is how
//!   the models shipped with spin itself are measured. Reports and exits; writes nothing.
//!
//! Exit codes: 0 pass, 1 wrong verdict, 2 no oracle. A gate that cannot reach pan exits 2 rather
//! than reporting success: an unmeasured corpus is not a passing corpus.

use std::path::{Path, PathBuf};
use std::process::Command;

use spin_rs::parity::{Ours, Outcome, RECORDED_PAN, classify, corpus};

fn main() {
    std::process::exit(run());
}

struct Row {
    name: String,
    template: &'static str,
    outcome: Outcome,
    ours: Ours,
    pan: Option<usize>,
}

fn run() -> i32 {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let baseline = args.iter().any(|a| a == "--baseline");
    let gate = args.iter().any(|a| a == "--gate");
    let filter = args
        .windows(2)
        .find(|w| w[0] == "--filter")
        .map(|w| w[1].clone());
    let dir = args
        .windows(2)
        .find(|w| w[0] == "--dir")
        .map(|w| w[1].clone());
    if let Some(path) = dir {
        return run_directory(Path::new(&path));
    }

    if baseline == gate {
        eprintln!("usage: parity_gate --baseline | --gate [--filter <substr>]");
        return 2;
    }

    if !oracle_available() {
        eprintln!(
            "no oracle: spin and gcc must be on PATH. Refusing to report success on an unmeasured \
             corpus."
        );
        return 2;
    }

    let root = std::env::temp_dir().join("spin_rs_parity");
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::create_dir_all(&root);

    let models: Vec<_> = corpus()
        .into_iter()
        .filter(|m| filter.as_ref().is_none_or(|f| m.name.contains(f.as_str())))
        .collect();

    println!("parity gate: {} models", models.len());
    let mut rows = Vec::with_capacity(models.len());
    for (i, m) in models.iter().enumerate() {
        let dir = root.join(&m.name);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("m.pml"), &m.source);
        let ours = ours_outcome(&m.source);
        let pan = pan_errors(&dir, (m.claim != "model").then_some("p"));
        let outcome = classify(ours, pan);
        if i % 16 == 0 {
            println!("  {} / {}", i + 1, models.len());
        }
        rows.push(Row {
            name: m.name.clone(),
            template: m.template,
            outcome,
            ours,
            pan,
        });
    }

    report(&rows);
    if baseline {
        write_baseline(&rows);
        return 0;
    }
    gate_result(&rows)
}

/// Run every `.pml` file under `path` instead of the generated corpus.
///
/// This is how the models that ship with spin itself are measured: `Examples/` in the Spin source
/// tree carries 78 of them, from the tool's own author, and `Examples/README_tests.txt` records
/// pan's expected results for a handful. The comparison is the same one the generated corpus gets -
/// verdict and error count per model - with one addition: a file carrying more than one claim is
/// not comparable from a single error count, so it is named as such instead of being guessed at.
fn run_directory(path: &Path) -> i32 {
    if !path.exists() {
        eprintln!("no such directory: {}", path.display());
        return 2;
    }
    if !oracle_available() {
        eprintln!("no oracle: spin and gcc must be on PATH.");
        return 2;
    }

    let mut files = Vec::new();
    collect_pml(path, &mut files);
    files.sort();
    if files.is_empty() {
        eprintln!("no .pml files under {}", path.display());
        return 2;
    }

    let root = std::env::temp_dir().join("spin_rs_parity_dir");
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::create_dir_all(&root);

    println!(
        "external corpus: {} models under {}",
        files.len(),
        path.display()
    );
    let mut rows = Vec::with_capacity(files.len());
    // Why each model we could not decide went undecided. Grouped at the end, because the list
    // of reasons is the work list: fifty refusals with one reason is one missing feature.
    let mut reasons: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    for (i, file) in files.iter().enumerate() {
        let Ok(source) = std::fs::read_to_string(file) else {
            continue;
        };
        let name = file
            .strip_prefix(path)
            .unwrap_or(file)
            .display()
            .to_string();
        let claims = count_claims(&source);
        let dir = root.join(format!("m{i}"));
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("m.pml"), &source);

        let (outcome, ours, pan) = if claims > 1 {
            (Outcome::NotComparable, Ours::Decline, None)
        } else {
            let (ours, why) = ours_outcome_capped(&source);
            if let Some(why) = why {
                reasons.entry(why).or_default().push(name.clone());
            }
            let pan = pan_errors(&dir, None);
            (classify(ours, pan), ours, pan)
        };
        if i % 10 == 0 {
            println!("  {} / {}", i + 1, files.len());
        }
        rows.push(Row {
            name,
            template: "external",
            outcome,
            ours,
            pan,
        });
    }

    if !reasons.is_empty() {
        println!("\nnot decided, by reason:");
        for (reason, names) in &reasons {
            println!("  {:>3}  {}", names.len(), reason);
            if names.len() <= 3 {
                for n in names {
                    println!("         {}", n);
                }
            }
        }
    }

    report(&rows);
    if rows.iter().any(|r| r.outcome.is_wrong()) {
        1
    } else {
        0
    }
}

fn collect_pml(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_pml(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("pml") {
            out.push(path);
        }
    }
}

/// `ltl` and `never` declarations, which each become a claim pan checks in its own run.
fn count_claims(source: &str) -> usize {
    source
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            t.starts_with("ltl ") || t.starts_with("ltl{") || t.starts_with("never")
        })
        .count()
}

/// Our verdict with a state cap, so one upstream model cannot hang the measurement.
fn ours_outcome_capped(source: &str) -> (Ours, Option<String>) {
    let cfg = spin_rs::CheckerConfig {
        max_states: 300_000,
        ..Default::default()
    };
    match spin_rs::verify_with_config(source, &cfg) {
        Ok(r) if r.unverified.is_empty() => (Ours::Verdict(r.errors), None),
        Ok(r) => (
            Ours::Decline,
            Some(format!(
                "declined: {}",
                r.unverified.first().map(|u| u.kind.as_str()).unwrap_or("?")
            )),
        ),
        Err(e) => (
            Ours::Refuse,
            Some(format!("refused: {}", first_line(&e.to_string()))),
        ),
    }
}

/// Everything before the first newline, so a multi-line diagnostic groups with its kind.
fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or("").to_string()
}

fn ours_outcome(source: &str) -> Ours {
    match spin_rs::verify(source) {
        Ok(r) if r.unverified.is_empty() => Ours::Verdict(r.errors),
        Ok(_) => Ours::Decline,
        Err(_) => Ours::Refuse,
    }
}

fn oracle_available() -> bool {
    let spin = Command::new("spin").arg("-V").output();
    let gcc = Command::new("gcc").arg("--version").output();
    matches!(spin, Ok(o) if o.status.success()) && matches!(gcc, Ok(o) if o.status.success())
}

/// pan's error count, or `None` when it cannot build or cannot run the model.
fn pan_errors(dir: &Path, claim: Option<&str>) -> Option<usize> {
    let built = Command::new("spin")
        .args(["-a", "m.pml"])
        .current_dir(dir)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !built {
        return None;
    }
    let compiled = Command::new("gcc")
        .args(["-w", "-o", "pan", "pan.c"])
        .current_dir(dir)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !compiled {
        return None;
    }
    let mut cmd = Command::new("./pan");
    match claim {
        Some(c) => {
            cmd.args(["-a", "-N", c]);
        }
        None => {
            cmd.arg("-n");
        }
    }
    let out = cmd.current_dir(dir).output().ok()?;
    parse_errors(&String::from_utf8_lossy(&out.stdout))
}

/// pan prints `errors: N` at the end of a line that also carries the state-vector size.
fn parse_errors(text: &str) -> Option<usize> {
    for line in text.lines() {
        if let Some(pos) = line.find("errors:") {
            let rest = line[pos + "errors:".len()..].trim_start();
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            if !digits.is_empty() {
                return digits.parse().ok();
            }
        }
    }
    None
}

fn report(rows: &[Row]) {
    let mut counts: Vec<(Outcome, usize)> = Vec::new();
    for o in [
        Outcome::Agree,
        Outcome::FalseRefutation,
        Outcome::FalseClean,
        Outcome::Decline,
        Outcome::Refusal,
        Outcome::PanUnbuildable,
    ] {
        let n = rows.iter().filter(|r| r.outcome == o).count();
        counts.push((o, n));
    }
    println!();
    for (o, n) in &counts {
        println!("  {:<18} {}", o.label(), n);
    }
    println!("  {:<18} {}", "total", rows.len());

    // An unbuildable pan and a decline are both models that were not compared. They are not
    // wrong verdicts, but a corpus that quietly loses models to them is a corpus that looks
    // better than it is, so they are named.
    let unmeasured: Vec<&Row> = rows
        .iter()
        .filter(|r| matches!(r.outcome, Outcome::PanUnbuildable | Outcome::Decline))
        .collect();
    if !unmeasured.is_empty() {
        println!("\nnot compared:");
        for r in unmeasured {
            println!("  {:<45} {}", r.name, r.outcome.label());
        }
    }

    let wrong: Vec<&Row> = rows.iter().filter(|r| r.outcome.is_wrong()).collect();
    if wrong.is_empty() {
        println!("\nno wrong verdicts");
        return;
    }
    println!("\nwrong verdicts:");
    for r in wrong {
        println!(
            "  {:<28} {} (ours {:?}, pan {:?})",
            r.name,
            r.outcome.label(),
            r.ours,
            r.pan
        );
    }

    for o in [Outcome::Refusal, Outcome::Decline] {
        let list: Vec<&Row> = rows.iter().filter(|r| r.outcome == o).collect();
        if list.is_empty() {
            continue;
        }
        println!("\n{}:", o.label());
        for r in list {
            println!("  {:<28} ours {:?}, pan {:?}", r.name, r.ours, r.pan);
        }
    }
}

/// The measured allowlist, one `name<TAB>outcome` per line.
///
/// Written by `--baseline`, read by `--gate`. Regenerating it is how progress is recorded: the
/// diff is the list of defects that stopped being wrong.
fn allowlist_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("openspec/changes/full-parity/known-wrong.txt")
}

fn read_allowlist() -> Vec<(String, String)> {
    let Ok(text) = std::fs::read_to_string(allowlist_path()) else {
        return Vec::new();
    };
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let (n, o) = l.split_once('\t')?;
            Some((n.trim().to_string(), o.trim().to_string()))
        })
        .collect()
}

fn gate_result(rows: &[Row]) -> i32 {
    let mut failures: Vec<String> = Vec::new();
    let mut stale: Vec<&str> = Vec::new();

    let allow = read_allowlist();
    for r in rows {
        let recorded = allow.iter().find(|(n, _)| *n == r.name);
        match (r.outcome.is_wrong(), recorded) {
            (true, None) => failures.push(format!(
                "{} is a {} and is not in the recorded baseline",
                r.name,
                r.outcome.label()
            )),
            (true, Some((_, expected))) if *expected != r.outcome.label() => {
                failures.push(format!(
                    "{} is a {} but is recorded as a {}",
                    r.name,
                    r.outcome.label(),
                    expected
                ))
            }
            (false, Some(_)) => stale.push(r.name.as_str()),
            _ => {}
        }
    }

    // A recorded pan verdict that no longer holds means the generator drifted away from the defect
    // it was built to reproduce, which is a failure of the instrument rather than of the checker.
    for (name, expected) in RECORDED_PAN {
        if let Some(r) = rows.iter().find(|r| r.name == *name)
            && let Some(live) = r.pan
            && live != *expected
        {
            failures.push(format!(
                "pan now reports {live} errors on {name}, recorded as {expected}: the generator no \
                 longer reproduces the recorded shape"
            ));
        }
    }

    for s in &stale {
        println!("\nno longer wrong: {s} - re-run --baseline to drop it from the allowlist");
    }

    if failures.is_empty() {
        println!("\ngate: pass");
        0
    } else {
        println!("\ngate: FAIL");
        for f in &failures {
            println!("  {f}");
        }
        1
    }
}

fn write_baseline(rows: &[Row]) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("openspec/changes/full-parity");
    let path = dir.join("baseline.md");
    let mut doc = String::new();
    doc.push_str("# Baseline\n\n");
    doc.push_str(
        "Generated by `cargo run --release --bin parity_gate -- --baseline`. One row per model;\n\
         one claim per model, so ours and pan are like for like.\n\n",
    );
    doc.push_str("| outcome | count |\n|---|---|\n");
    for o in [
        Outcome::Agree,
        Outcome::FalseRefutation,
        Outcome::FalseClean,
        Outcome::Decline,
        Outcome::Refusal,
        Outcome::PanUnbuildable,
    ] {
        let n = rows.iter().filter(|r| r.outcome == o).count();
        doc.push_str(&format!("| {} | {} |\n", o.label(), n));
    }
    doc.push_str(&format!("| total | {} |\n\n", rows.len()));

    // The machine allowlist. Regenerating it is how progress is recorded.
    let mut allow = String::from("# name<TAB>outcome - written by parity_gate --baseline\n");
    for r in rows.iter().filter(|r| r.outcome.is_wrong()) {
        allow.push_str(&format!("{}\t{}\n", r.name, r.outcome.label()));
    }
    let allow_path = allowlist_path();
    if let Err(e) = std::fs::write(&allow_path, &allow) {
        eprintln!("could not write {}: {e}", allow_path.display());
    }

    let wrong: Vec<&Row> = rows.iter().filter(|r| r.outcome.is_wrong()).collect();
    doc.push_str("## Wrong verdicts\n\n");
    if wrong.is_empty() {
        doc.push_str("None.\n");
    } else {
        doc.push_str("| model | outcome | ours | pan |\n|---|---|---|---|\n");
        for r in &wrong {
            doc.push_str(&format!(
                "| `{}` | {} | {:?} | {:?} |\n",
                r.name,
                r.outcome.label(),
                r.ours,
                r.pan
            ));
        }
    }

    doc.push_str("\n## Per template\n\n| template | agree | wrong | other |\n|---|---|---|---|\n");
    let mut templates: Vec<&str> = rows.iter().map(|r| r.template).collect();
    templates.sort_unstable();
    templates.dedup();
    for t in templates {
        let of: Vec<&Row> = rows.iter().filter(|r| r.template == t).collect();
        let agree = of.iter().filter(|r| r.outcome == Outcome::Agree).count();
        let wrong = of.iter().filter(|r| r.outcome.is_wrong()).count();
        doc.push_str(&format!(
            "| `{t}` | {agree} | {wrong} | {} |\n",
            of.len() - agree - wrong
        ));
    }

    if let Err(e) = std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&path, &doc)) {
        eprintln!("could not write {}: {e}", path.display());
    } else {
        println!("\nbaseline written to {}", path.display());
    }
}
