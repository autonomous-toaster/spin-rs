use spin_rs::parser;

const VAR: &str = "byte x = 0;";
const PROC: &str = "active proctype P() { do :: x = 0 :: x = 1 od }";
const LTL: &str = "ltl p0 { [](x == 0) }";

fn test_parse(name: &str, source: &str) {
    println!("=== {} ===", name);
    match parser::parse(source) {
        Ok(ast) => {
            println!("✓ Parsed {} declarations", ast.declarations.len());
        }
        Err(e) => println!("✗ Parse error: {}", e),
    }
    println!();
}

fn main() {
    test_parse("VAR", VAR);
    test_parse("PROC", PROC);
    test_parse("LTL", LTL);
    test_parse("VAR + PROC", &format!("{}\n{}", VAR, PROC));
    test_parse("PROC + LTL", &format!("{}\n{}", PROC, LTL));
    test_parse("VAR + PROC + LTL", &format!("{}\n{}\n{}", VAR, PROC, LTL));
}
