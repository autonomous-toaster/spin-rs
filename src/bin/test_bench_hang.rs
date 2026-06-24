use std::process::{Command, Stdio};
use std::time::Instant;

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

fn run_spin(source: &str) -> Option<()> {
    let tmp = std::env::temp_dir().join("spin_bench_hang");
    let _ = std::fs::create_dir_all(&tmp);
    std::fs::write(tmp.join("model.pml"), source).ok()?;

    println!("Step 1: spin -a (timeout 5s)...");
    let start = Instant::now();
    let o1 = Command::new("spin")
        .arg("-a")
        .arg(tmp.join("model.pml"))
        .current_dir(&tmp)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .ok()?;
    println!(
        "  spin -a took {:?}, success={}",
        start.elapsed(),
        o1.status.success()
    );
    if !o1.status.success() {
        println!("  stderr: {}", String::from_utf8_lossy(&o1.stderr));
        return None;
    }

    println!("Step 2: gcc (timeout 5s)...");
    let start = Instant::now();
    let o2 = Command::new("gcc")
        .args(["-O2", "-o", "pan", "pan.c"])
        .current_dir(&tmp)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .ok()?;
    println!(
        "  gcc took {:?}, success={}",
        start.elapsed(),
        o2.status.success()
    );
    if !o2.status.success() {
        println!("  stderr: {}", String::from_utf8_lossy(&o2.stderr));
        return None;
    }

    println!("Step 3: ./pan -n (timeout 10s)...");
    let start = Instant::now();
    let o3 = Command::new("./pan")
        .arg("-n")
        .current_dir(&tmp)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .ok()?;
    println!(
        "  pan took {:?}, success={}",
        start.elapsed(),
        o3.status.success()
    );
    if !o3.status.success() {
        println!("  stderr: {}", String::from_utf8_lossy(&o3.stderr));
        return None;
    }

    println!("Output:\n{}", String::from_utf8_lossy(&o3.stdout));
    Some(())
}

fn main() {
    println!("Testing Spin execution (mimicking benchmark)...");
    let start = Instant::now();
    run_spin(PLAN_5TASKS_3LTLS);
    println!("Total time: {:?}", start.elapsed());
}
