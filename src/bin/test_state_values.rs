use spin_rs::{engine::checker::Model, runtime::LuaModel};

const LTL_VIOLATION: &str = r#"byte x = 0;
active proctype P() { do :: x = 0 :: x = 1 od }
ltl p0 { [](x == 0) }
"#;

fn main() {
    println!("=== Testing State Values ===");
    let model = LuaModel::from_source(LTL_VIOLATION).unwrap();
    let init_states = model.init_states();

    println!("Initial states: {}", init_states.len());
    for (i, state) in init_states.iter().enumerate() {
        println!("State {}: {}", i, state.0);
    }

    if !init_states.is_empty() {
        let transitions = model.transitions(&init_states[0]);
        println!("Transitions from initial: {}", transitions.len());
        for (j, t) in transitions.iter().enumerate() {
            println!("  {}: {} -> {}", j, t.label, t.next.0);
        }
    }
}
