use spin_rs::parser;

const TEST1: &str = "active proctype P() { skip; skip }";
const TEST2: &str = "active proctype P() { skip; skip; }";
const TEST3: &str = "active proctype P() { skip skip }";

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
    test_parse("TEST1 (skip; skip)", TEST1);
    test_parse("TEST2 (skip; skip;)", TEST2);
    test_parse("TEST3 (skip skip)", TEST3);
}
