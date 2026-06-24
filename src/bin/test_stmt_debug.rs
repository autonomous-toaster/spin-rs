use spin_rs::parser;

// Test parsing assignment in different contexts
const TEST1: &str = "active proctype P() { x = 0 }";
const TEST2: &str = "active proctype P() { do :: x = 0 od }";
const TEST3: &str = "active proctype P() { do :: (x == 0) -> x = 0 od }";

fn test(name: &str, source: &str) {
    println!("=== {} ===", name);
    match parser::parse(source) {
        Ok(ast) => {
            println!("✓ {} decls", ast.declarations.len());
            for decl in &ast.declarations {
                if let spin_rs::parser::ast::TopLevel::Proctype(p) = decl {
                    println!("  Body has {} statements", p.body.len());
                    for (i, stmt) in p.body.iter().enumerate() {
                        println!("    {}: {:?}", i, stmt);
                    }
                }
            }
        }
        Err(e) => println!("✗ {}", e),
    }
}

fn main() {
    test("direct", TEST1);
    test("in do", TEST2);
    test("with guard", TEST3);
}
