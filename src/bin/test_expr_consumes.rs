use spin_rs::parser;

// Test if expr parser consumes the full assignment
const TEST1: &str = "active proctype P() { x }";
const TEST2: &str = "active proctype P() { x = 0 }";
const TEST3: &str = "active proctype P() { (x) }";
const TEST4: &str = "active proctype P() { (x = 0) }";

fn test_parse(name: &str, source: &str) {
    println!("=== {} ===", name);
    match parser::parse(source) {
        Ok(ast) => {
            println!("✓ Parsed {} declarations", ast.declarations.len());
            for (i, decl) in ast.declarations.iter().enumerate() {
                println!("  {}: {:?}", i, decl);
            }
        }
        Err(e) => println!("✗ Parse error: {}", e),
    }
    println!();
}

fn main() {
    test_parse("TEST1 (x)", TEST1);
    test_parse("TEST2 (x = 0)", TEST2);
    test_parse("TEST3 ((x))", TEST3);
    test_parse("TEST4 ((x = 0))", TEST4);
}
