use spin_rs::parser;

const TEST1: &str = "active proctype P() { x = 0 }";
const TEST2: &str = "active proctype P() { x = 0; }";
const TEST3: &str = "active proctype P() { x = 0; y = 1 }";
const TEST4: &str = "active proctype P() { x = 0; y = 1; }";

fn test_parse(name: &str, source: &str) {
    println!("=== {} ===", name);
    match parser::parse(source) {
        Ok(ast) => {
            println!("Parsed {} declarations", ast.declarations.len());
        }
        Err(e) => println!("Parse error: {}", e),
    }
    println!();
}

fn main() {
    test_parse("TEST1 (x = 0)", TEST1);
    test_parse("TEST2 (x = 0;)", TEST2);
    test_parse("TEST3 (x = 0; y = 1)", TEST3);
    test_parse("TEST4 (x = 0; y = 1;)", TEST4);
}
