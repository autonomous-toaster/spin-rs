use spin_rs::parser;

const TEST1: &str = "active proctype P() { x = 0 }";
const TEST2: &str = "active proctype P() { x = 0; }";
const TEST3: &str = "active proctype P() { do :: skip od }";

fn test(name: &str, source: &str) {
    println!("=== {} ===", name);
    match parser::parse(source) {
        Ok(ast) => println!("✓ {} decls", ast.declarations.len()),
        Err(e) => println!("✗ {}", e),
    }
}

fn main() {
    test("x = 0", TEST1);
    test("x = 0;", TEST2);
    test("do :: skip od", TEST3);
}
