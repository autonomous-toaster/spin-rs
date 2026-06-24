use spin_rs::parser;

// Exact from bench_vs_spin.rs
const LTL_VIOLATION: &str = r#"byte x = 0;
active proctype P() { do :: x = 0 :: x = 1 od }
ltl p0 { [](x == 0) }
"#;

fn main() {
    println!("=== Parsing EXACT LTL_VIOLATION from bench ===");
    match parser::parse(LTL_VIOLATION) {
        Ok(ast) => {
            println!("✓ Parsed {} declarations", ast.declarations.len());
            for (i, decl) in ast.declarations.iter().enumerate() {
                match decl {
                    spin_rs::parser::ast::TopLevel::Ltl(l) => {
                        println!("  {}: Ltl name={:?}, formula='{}'", i, l.name, l.formula);
                    }
                    _ => {
                        println!("  {}: {:?}", i, decl);
                    }
                }
            }
        }
        Err(e) => println!("✗ Parse error: {}", e),
    }
}
