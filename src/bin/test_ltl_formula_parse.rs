use spin_rs::property::ltl2ba::parser::parse_ltl;

fn main() {
    println!("=== Testing ltl2ba Parser ===");

    let formulas = vec!["[](x == 0)", "[](x)", "<>(x == 1)", "x == 0"];

    for formula_str in formulas {
        println!("\nFormula: {}", formula_str);
        match parse_ltl(formula_str) {
            Ok(f) => println!("  Parsed: {:?}", f),
            Err(e) => println!("  Error: {}", e),
        }
    }
}
