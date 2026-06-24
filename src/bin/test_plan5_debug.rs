use spin_rs::{SearchMode, engine::checker::CheckerBuilder, runtime::LuaModel};

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
    println!("Testing plan_5tasks_3ltls with different configs...");
    let _model = LuaModel::from_source(PLAN_5TASKS_3LTLS).unwrap();

    for (name, search, por) in &[
        ("DFS noPOR", SearchMode::DepthFirst, false),
        ("DFS POR", SearchMode::DepthFirst, true),
        ("BFS noPOR", SearchMode::BreadthFirst, false),
    ] {
        println!("\nRunning {}...", name);
        let checker = CheckerBuilder::new()
            .model(LuaModel::from_source(PLAN_5TASKS_3LTLS).unwrap())
            .search_mode(*search)
            .por_enabled(*por)
            .max_states(1_000_000)
            .max_depth(100_000)
            .build();
        let result = checker.check();
        println!(
            "  States: {}, Errors: {}, Transitions: {}",
            result.states_explored, result.errors, result.transitions
        );
    }
}
