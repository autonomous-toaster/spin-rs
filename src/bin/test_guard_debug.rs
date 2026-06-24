use spin_rs::parser;

// Debug: what happens in guard_body?
const TEST: &str = "active proctype P() { do :: x = 0 od }";

fn main() {
    println!("=== Parsing: {} ===", TEST);
    match parser::parse(TEST) {
        Ok(ast) => {
            println!("✓ {} decls", ast.declarations.len());
            for decl in &ast.declarations {
                if let spin_rs::parser::ast::TopLevel::Proctype(p) = decl {
                    println!("  Body: {} stmts", p.body.len());
                    for (i, stmt) in p.body.iter().enumerate() {
                        println!("    {}: {:?}", i, stmt);
                    }
                }
            }
        }
        Err(e) => println!("✗ Parse error: {}", e),
    }
}
