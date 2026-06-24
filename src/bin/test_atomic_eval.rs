use spin_rs::property::ltl2ba::product::evaluate_atomic_props;
use spin_rs::{engine::checker::Model, runtime::LuaModel};

const LTL_VIOLATION: &str = r#"byte x = 0;
active proctype P() { do :: x = 0 :: x = 1 od }
ltl p0 { [](x == 0) }
"#;

fn main() {
    println!("=== Testing Atomic Prop Evaluation ===");
    let model = LuaModel::from_source(LTL_VIOLATION).unwrap();
    let init_states = model.init_states();

    if !init_states.is_empty() {
        let state = &init_states[0];
        println!("State: {}", state.0);

        let props = evaluate_atomic_props(&model, state);
        println!("Atomic props: {:?}", props);

        // Check if "x == 0" is evaluated
        if let Some(val) = props.get("x == 0") {
            println!("'x == 0' = {}", val);
        }

        // Check if "x" is evaluated
        if let Some(val) = props.get("x") {
            println!("'x' = {}", val);
        }
    }
}
