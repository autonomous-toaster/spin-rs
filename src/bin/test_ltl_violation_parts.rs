use spin_rs::parser;

const PART1: &str = "byte x = 0;";
const PART2: &str = "active proctype P() { do :: x = 0 :: x = 1 od }";
const PART3: &str = "ltl p0 { [](x == 0) }";
const COMBINED: &str = "byte x = 0;
active proctype P() { do :: x = 0 :: x = 1 od }
ltl p0 { [](x == 0) }
";

fn test_parse(name: &str, source: &str) {
    println!("=== {} ===", name);
    match parser::parse(source) {
        Ok(ast) => {
            println!("Parsed {} declarations", ast.declarations.len());
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
        Err(e) => println!("Parse error: {}", e),
    }
    println!();
}

fn main() {
    test_parse("PART1 (var)", PART1);
    test_parse("PART2 (proctype)", PART2);
    test_parse("PART3 (LTL)", PART3);
    test_parse("COMBINED", COMBINED);
}
