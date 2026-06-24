use spin_rs::parser;

// Test different guard syntaxes
const TEST1: &str = "active proctype P() { do :: (x == 0) -> x = 1 od }"; // Condition with arrow
const TEST2: &str = "active proctype P() { do :: x = 1 od }"; // No condition, just statement
const TEST3: &str = "active proctype P() { do :: -> x = 1 od }"; // Explicit empty condition
const TEST4: &str = "active proctype P() { do :: skip od }"; // Simple statement

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
    test_parse("TEST1 (condition -> stmt)", TEST1);
    test_parse("TEST2 (stmt only)", TEST2);
    test_parse("TEST3 (-> stmt)", TEST3);
    test_parse("TEST4 (skip)", TEST4);
}
