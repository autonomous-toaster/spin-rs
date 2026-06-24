use spin_rs::parser;

const TEST1: &str = "active proctype P() { do :: skip :: skip od }";
const TEST2: &str = "active proctype P() { do :: skip od }";
const TEST3: &str = "active proctype P() { do :: (x == 0) -> skip od }";
const TEST4: &str = "active proctype P() { do :: x = 0 od }";
const TEST5: &str = "active proctype P() { do :: x = 0; od }";

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
    test_parse("TEST1 (skip skip)", TEST1);
    test_parse("TEST2 (skip)", TEST2);
    test_parse("TEST3 (guard -> skip)", TEST3);
    test_parse("TEST4 (assign no ;)", TEST4);
    test_parse("TEST5 (assign with ;)", TEST5);
}
