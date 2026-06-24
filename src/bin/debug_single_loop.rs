use spin_rs::{codegen, parser};

const SINGLE_LOOP: &str = r#"
active proctype P() {
    byte x = 0;
    do :: x < 100 -> x = x + 1 :: x >= 100 -> break od
}
"#;

fn main() {
    let ast = parser::parse(SINGLE_LOOP).unwrap();
    let lua = codegen::generate(&ast);
    println!("{}", lua.source);
}
