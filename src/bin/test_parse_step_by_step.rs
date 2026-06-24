use spin_rs::parser;

const TEST1: &str = "byte x = 0;";
const TEST2: &str = "active proctype P() { do :: x = 0 :: x = 1 od }";
const TEST3: &str = "ltl p0 { [](x == 0) }";
const TEST4: &str = "byte x = 0;
active proctype P() { do :: x = 0 :: x = 1 od }";
const TEST5: &str = "active proctype P() { do :: x = 0 :: x = 1 od }
ltl p0 { [](x == 0) }";

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
    test_parse("TEST1 (var)", TEST1);
    test_parse("TEST2 (proctype)", TEST2);
    test_parse("TEST3 (LTL)", TEST3);
    test_parse("TEST4 (var + proctype)", TEST4);
    test_parse("TEST5 (proctype + LTL)", TEST5);
}
