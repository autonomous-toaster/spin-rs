use spin_rs::parser;

// Test statement parsing in guards
const TEST1: &str = "active proctype P() { do :: x = 0 od }";
const TEST2: &str = "active proctype P() { do :: (x == 0) -> x = 0 od }";
const TEST3: &str = "active proctype P() { do :: skip od }";

fn test_parse(name: &str, source: &str) {
    println!("=== {} ===", name);
    match parser::parse(source) {
        Ok(ast) => {
            println!("✓ Parsed {} declarations", ast.declarations.len());
        }
        Err(e) => println!("✗ Parse error: {}", e),
    }
    println!();
}

fn main() {
    test_parse("TEST1 (x = 0)", TEST1);
    test_parse("TEST2 ((x == 0) -> x = 0)", TEST2);
    test_parse("TEST3 (skip)", TEST3);
}
