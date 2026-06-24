use spin_rs::parser;

const TEST1: &str = "active proctype P() { skip }";
const TEST2: &str = "active proctype P() { do :: skip od }";

fn test_parse(name: &str, source: &str) {
    println!("=== {} ===", name);
    match parser::parse(source) {
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
    test_parse("TEST1 (simple)", TEST1);
    test_parse("TEST2 (do loop)", TEST2);
}
