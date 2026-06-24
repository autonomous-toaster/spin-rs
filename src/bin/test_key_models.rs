use spin_rs::{engine::checker::CheckerBuilder, runtime::LuaModel};

const DEADLOCK_CIRCULAR: &str = r#"
chan ch1 = [0] of { byte }; chan ch2 = [0] of { byte };
active proctype P() { ch1 ! 1; ch2 ? 0; }
active proctype Q() { ch2 ! 1; ch1 ? 0; }
"#;

const SINGLE_LOOP: &str = r#"
active proctype P() {
    byte x = 0;
    do :: x < 100 -> x = x + 1 :: x >= 100 -> break od
}
"#;

const LTL_VIOLATION: &str = r#"
bool p = false;
active proctype P() { p = true; }
ltl { <>p }
"#;

fn test_model(name: &str, source: &str, expected_states: Option<usize>) {
    let model = LuaModel::from_source(source).unwrap();
    let checker = CheckerBuilder::new().model(model).build();
    let result = checker.check_dfs();
    print!("{}: {} states", name, result.states_explored);
    if let Some(exp) = expected_states {
        if result.states_explored == exp {
            print!(" ✓");
        } else {
            print!(" ✗ (expected {})", exp);
        }
    }
    println!();
}

fn main() {
    println!("Testing key models:");
    test_model("deadlock_circular", DEADLOCK_CIRCULAR, None);
    test_model("single_loop", SINGLE_LOOP, Some(102));
    test_model("ltl_violation", LTL_VIOLATION, None);
}
