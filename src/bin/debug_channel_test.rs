use spin_rs::{
    engine::checker::{CheckerBuilder, Model, SearchMode},
    runtime::LuaModel,
};

const CHANNEL_TEST: &str = r#"
chan q = [1] of { byte };

active proctype Sender() {
    q ! 1;
    q ! 2;
}

active proctype Receiver() {
    byte x;
    q ? x;
    q ? x;
}
"#;

fn main() {
    let model = LuaModel::from_source(CHANNEL_TEST).unwrap();
    println!("Init state: {}", model.init_state().0);

    let checker = CheckerBuilder::new()
        .search_mode(SearchMode::BFS)
        .model(model)
        .build();
    let result = checker.check_dfs();
    println!("States: {}", result.states_explored);
    println!("Errors: {}", result.errors);
    if result.errors > 0 {
        for e in &result.error_traces {
            println!("Error trace: ");
            for s in &e.trace {
                println!("  {}", s.0);
            }
            println!("  desc: {}", e.description);
        }
    }
}
