use spin_rs::parser;

const TEST1: &str = "ltl p0 { [](x == 0) }";
const TEST2: &str = "byte x = 0; ltl p0 { [](x == 0) }";
const TEST3: &str = "byte x = 0;
ltl p0 { [](x == 0) }";

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
    test_parse("TEST1 (LTL only)", TEST1);
    test_parse("TEST2 (var + LTL same line)", TEST2);
    test_parse("TEST3 (var + LTL multi-line)", TEST3);
}
