use spin_rs::parser;

const TEST1: &str = "x = 0";
const TEST2: &str = "(x == 0)";
const TEST3: &str = "x";

fn test_expr(name: &str, source: &str) {
    println!("=== {} ===", name);
    match parser::parse(&format!("active proctype P() {{ {} }}", source)) {
        Ok(ast) => {
            println!("Parsed {} declarations", ast.declarations.len());
            for (i, decl) in ast.declarations.iter().enumerate() {
                println!("  {}: {:?}", i, decl);
            }
        }
        Err(e) => println!("Parse error: {}", e),
    }
    println!();
}

fn main() {
    test_expr("TEST1 (assign)", TEST1);
    test_expr("TEST2 (cond)", TEST2);
    test_expr("TEST3 (ident)", TEST3);
}
