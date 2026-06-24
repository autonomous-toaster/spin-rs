use spin_rs::parser;

const TEST1: &str = "active proctype P() { do :: x = 0 od }";
const TEST2: &str = "active proctype P() { do :: x = 0 :: x = 1 od }";
const TEST3: &str = "byte x = 0;
active proctype P() { do :: x = 0 :: x = 1 od }
ltl p0 { [](x == 0) }
";

fn test_parse(name: &str, source: &str) {
    println!("=== {} ===", name);
    match parser::parse(source) {
        Ok(ast) => {
            println!("✓ Parsed {} declarations", ast.declarations.len());
            for (i, decl) in ast.declarations.iter().enumerate() {
                if let spin_rs::parser::ast::TopLevel::Ltl(l) = decl {
                    println!("  {}: Ltl name={:?}, formula='{}'", i, l.name, l.formula);
                }
            }
        }
        Err(e) => println!("✗ Parse error: {}", e),
    }
    println!();
}

fn main() {
    test_parse("TEST1 (do :: x = 0 od)", TEST1);
    test_parse("TEST2 (do :: x = 0 :: x = 1 od)", TEST2);
    test_parse("TEST3 (LTL_VIOLATION)", TEST3);
}
