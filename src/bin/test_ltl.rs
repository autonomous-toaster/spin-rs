use spin_rs::{engine::checker::CheckerBuilder, runtime::LuaModel};

const LTL_VIOLATION: &str = r#"
byte x = 0;
active proctype P() { do :: x = 0 :: x = 1 od }
ltl p0 { [](x == 0) }
"#;

fn main() {
    println!("Testing LTL model...");
    let model = LuaModel::from_source(LTL_VIOLATION).unwrap();
    println!("Model created");
    let checker = CheckerBuilder::new().model(model).build();
    println!("Checker built");
    let result = checker.check_dfs();
    println!("States explored: {}", result.states_explored);
    println!("Errors: {}", result.errors);
}
