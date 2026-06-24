use spin_rs::{engine::checker::CheckerBuilder, runtime::LuaModel};

const SINGLE_LOOP: &str = r#"
active proctype P() {
    byte x = 0;
    do :: x < 100 -> x = x + 1 :: x >= 100 -> break od
}
"#;

fn main() {
    let model = LuaModel::from_source(SINGLE_LOOP).unwrap();
    let checker = CheckerBuilder::new().model(model).build();
    let result = checker.check_dfs();
    println!("States explored: {}", result.states_explored);
    println!("Errors: {}", result.errors);
}
