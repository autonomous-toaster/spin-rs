use spin_rs::parser;

const LTL_VIOLATION: &str = r#"
byte x = 0;
active proctype P() { do :: x = 0 :: x = 1 od }
ltl p0 { [](x == 0) }
"#;

fn main() {
    println!("=== Parsing LTL model ===");
    let ast = parser::parse(LTL_VIOLATION).unwrap();
    println!("Parsed {} declarations:", ast.declarations.len());

    for (i, decl) in ast.declarations.iter().enumerate() {
        match decl {
            spin_rs::parser::ast::TopLevel::GlobalVar(v) => {
                println!("  {}: GlobalVar {} = {:?}", i, v.name, v.init);
            }
            spin_rs::parser::ast::TopLevel::Proctype(p) => {
                println!("  {}: Proctype {}", i, p.name);
            }
            spin_rs::parser::ast::TopLevel::Ltl(l) => {
                println!("  {}: Ltl name={:?}, formula='{}'", i, l.name, l.formula);
            }
            _ => {
                println!("  {}: {:?}", i, decl);
            }
        }
    }
}
