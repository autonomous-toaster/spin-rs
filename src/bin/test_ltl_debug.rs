use spin_rs::property::LtlFormula;
use spin_rs::{parser, runtime::LuaModel};

const LTL_VIOLATION: &str = r#"
byte x = 0;
active proctype P() { do :: x = 0 :: x = 1 od }
ltl p0 { [](x == 0) }
"#;

fn main() {
    println!("=== Testing LTL Parsing ===");
    let ast = parser::parse(LTL_VIOLATION).unwrap();
    println!("Parsed {} declarations", ast.declarations.len());

    for decl in &ast.declarations {
        if let spin_rs::parser::ast::TopLevel::Ltl(ltl) = decl {
            println!(
                "LTL formula: name={:?}, formula='{}'",
                ltl.name, ltl.formula
            );

            // Try to parse the formula
            match LtlFormula::parse(&ltl.formula) {
                Ok(f) => println!("  Parsed formula: {:?}", f),
                Err(e) => println!("  Parse error: {}", e),
            }
        }
    }

    println!("\n=== Testing LTL Model ===");
    let model = LuaModel::from_source(LTL_VIOLATION).unwrap();
    println!("LTL formulas in model: {}", model.ltl_formulas().len());

    for ltl in model.ltl_formulas() {
        println!("  LTL: name={:?}, formula='{}'", ltl.name, ltl.formula);
    }
}
