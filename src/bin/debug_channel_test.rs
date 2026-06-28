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
    let init_states = model.init_states();
    if !init_states.is_empty() {
        println!("Init state: {}", init_states[0].0);
    }

    let checker = CheckerBuilder::new()
        .search_mode(SearchMode::BreadthFirst)
        .model(model)
        .build();
    let result = checker.check_dfs();
    println!("States: {}", result.states_explored);
    println!("Errors: {}", result.errors);
    if result.errors > 0 {
        for v in &result.violations {
            println!("Violation: {} ({}):", v.property_name, v.description);
            for s in &v.trail {
                println!("  {}", s);
            }
        }
    }
}
