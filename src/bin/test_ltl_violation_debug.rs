use spin_rs::parser;

// Exact copy from bench_vs_spin.rs
const LTL_VIOLATION: &str = r#"byte x = 0;
active proctype P() { do :: x = 0 :: x = 1 od }
ltl p0 { [](x == 0) }
"#;

fn main() {
    println!("=== Parsing LTL_VIOLATION (EXACT from bench) ===");
    println!("Source:\n{}", LTL_VIOLATION);
    println!();

    match parser::parse(LTL_VIOLATION) {
        Ok(ast) => {
            println!("Parsed {} declarations", ast.declarations.len());
            for (i, decl) in ast.declarations.iter().enumerate() {
                println!("  {}: {:?}", i, decl);
            }
        }
        Err(e) => println!("Parse error: {}", e),
    }
}
