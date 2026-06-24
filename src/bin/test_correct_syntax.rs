use spin_rs::parser;

// Correct Promela syntax: guards need -> separator
const TEST1: &str = "active proctype P() { do :: (x == 0) -> x = 0 :: (x == 1) -> x = 1 od }";
const TEST2: &str = "active proctype P() { do :: x = 0 :: x = 1 od }"; // No guard, just statements
const TEST3: &str = "active proctype P() { do :: -> x = 0 :: -> x = 1 od }"; // Explicit no-guard

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
    test_parse("TEST1 (guards with ->)", TEST1);
    test_parse("TEST2 (no guards)", TEST2);
    test_parse("TEST3 (explicit no-guard)", TEST3);
}
