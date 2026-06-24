use spin_rs::{engine::checker::CheckerBuilder, runtime::LuaModel};

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

fn main() {
    println!("Testing plan_5tasks_3ltls...");
    let model = LuaModel::from_source(PLAN_5TASKS_3LTLS).unwrap();
    println!("Model created");
    let checker = CheckerBuilder::new().model(model).build();
    println!("Checker built, running...");
    let result = checker.check_dfs();
    println!("States explored: {}", result.states_explored);
    println!("Errors: {}", result.errors);
}
