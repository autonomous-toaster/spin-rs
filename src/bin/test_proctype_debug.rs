use spin_rs::parser;

const TEST1: &str = "active proctype P() { skip }";
const TEST2: &str = "active proctype P() { do :: skip od }";
const TEST3: &str = "active proctype P() { do :: x = 0 :: x = 1 od }";
const TEST4: &str = "active proctype P() { do :: x = 0 od }";

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
    test_parse("TEST1 (skip)", TEST1);
    test_parse("TEST2 (do skip)", TEST2);
    test_parse("TEST3 (do x=0 x=1)", TEST3);
    test_parse("TEST4 (do x=0)", TEST4);
}
