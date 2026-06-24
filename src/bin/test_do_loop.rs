use spin_rs::parser;

const TEST1: &str = "active proctype P() { do :: skip od }";
const TEST2: &str = "active proctype P() { do :: x = 0 od }";
const TEST3: &str = "active proctype P() { do :: x = 0 :: x = 1 od }";
const TEST4: &str = "active proctype P() { do :: (x == 0) -> x = 0 od }";

fn test(name: &str, source: &str) {
    println!("=== {} ===", name);
    match parser::parse(source) {
        Ok(ast) => println!("✓ {} decls", ast.declarations.len()),
        Err(e) => println!("✗ {}", e),
    }
}

fn main() {
    test("skip", TEST1);
    test("x = 0", TEST2);
    test("x = 0 :: x = 1", TEST3);
    test("(x==0)->x=0", TEST4);
}
