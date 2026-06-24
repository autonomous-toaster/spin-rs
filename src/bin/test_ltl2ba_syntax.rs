use spin_rs::property::ltl2ba::parser::parse_ltl;

fn main() {
    println!("=== Testing ltl2ba Parser Syntax ===");

    let formulas = vec!["[]p", "<>p", "p", "p && q", "[]x_eq_0"];

    for formula_str in formulas {
        print!("{}: ", formula_str);
        match parse_ltl(formula_str) {
            Ok(f) => println!("{:?}", f),
            Err(e) => println!("Error: {}", e),
        }
    }
}
