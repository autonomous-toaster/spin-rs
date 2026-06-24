use spin_rs::parser;

const TEST1: &str = "active proctype P() { do :: x = 0 :: x = 1 od }";
const TEST2: &str = "proctype P() { do :: x = 0 :: x = 1 od }";
const TEST3: &str = "active [1] proctype P() { do :: x = 0 :: x = 1 od }";

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
    test_parse("TEST1 (active proctype)", TEST1);
    test_parse("TEST2 (proctype)", TEST2);
    test_parse("TEST3 (active [1] proctype)", TEST3);
}
