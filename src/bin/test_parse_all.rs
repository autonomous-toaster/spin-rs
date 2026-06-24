use spin_rs::parser;

const TEST: &str = "byte x = 0;
active proctype P() { do :: x = 0 :: x = 1 od }
ltl p0 { [](x == 0) }";

fn main() {
    println!("=== Testing full parse ===");
    match parser::parse(TEST) {
        Ok(ast) => {
            println!("Parsed {} declarations", ast.declarations.len());
            for (i, decl) in ast.declarations.iter().enumerate() {
                println!("  {}: {:?}", i, decl);
            }
        }
        Err(e) => println!("Parse error: {}", e),
    }
}
