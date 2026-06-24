use spin_rs::parser;

// Test statement parsing order
const TEST1: &str = "active proctype P() { x = 0 }";
const TEST2: &str = "active proctype P() { skip }";
const TEST3: &str = "active proctype P() { do :: skip od }";

fn test(name: &str, source: &str) {
    match parser::parse(source) {
        Ok(ast) => println!("✓ {}: {} decls", name, ast.declarations.len()),
        Err(e) => println!("✗ {}: {}", name, e),
    }
}

fn main() {
    test("assign", TEST1);
    test("skip", TEST2);
    test("do skip", TEST3);
}
