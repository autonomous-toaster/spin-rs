use spin_rs::property::LtlFormula;
use spin_rs::property::ltl2ba::formula::LtlFormula as Ltl2baFormula;

fn main() {
    println!("=== Testing LTL Formula Conversion ===");

    // Parse the formula
    let formula = LtlFormula::parse("[](x == 0)").unwrap();
    println!("Parsed formula: {:?}", formula);

    // Convert to ltl2ba format
    let converted = convert_to_ltl2ba(&formula);
    println!("Converted: {:?}", converted);

    // Try to convert to Büchi
    match spin_rs::property::ltl2ba::to_buchi(&converted) {
        Ok(buchi) => {
            println!(
                "Büchi automaton: {} states, {} accepting",
                buchi.num_states,
                buchi.accepting.len()
            );
        }
        Err(e) => println!("Büchi conversion error: {}", e),
    }
}

fn convert_to_ltl2ba(f: &LtlFormula) -> Ltl2baFormula {
    match f {
        LtlFormula::True => Ltl2baFormula::True,
        LtlFormula::False => Ltl2baFormula::False,
        LtlFormula::Atom(s) => Ltl2baFormula::Atom(s.clone()),
        LtlFormula::Not(inner) => Ltl2baFormula::Not(Box::new(convert_to_ltl2ba(inner))),
        LtlFormula::And(l, r) => Ltl2baFormula::And(
            Box::new(convert_to_ltl2ba(l)),
            Box::new(convert_to_ltl2ba(r)),
        ),
        LtlFormula::Or(l, r) => Ltl2baFormula::Or(
            Box::new(convert_to_ltl2ba(l)),
            Box::new(convert_to_ltl2ba(r)),
        ),
        LtlFormula::Implies(l, r) => {
            let not_l = Ltl2baFormula::Not(Box::new(convert_to_ltl2ba(l)));
            Ltl2baFormula::Or(Box::new(not_l), Box::new(convert_to_ltl2ba(r)))
        }
        LtlFormula::Always(inner) => Ltl2baFormula::Always(Box::new(convert_to_ltl2ba(inner))),
        LtlFormula::Eventually(inner) => {
            Ltl2baFormula::Eventually(Box::new(convert_to_ltl2ba(inner)))
        }
        LtlFormula::Next(inner) => Ltl2baFormula::Next(Box::new(convert_to_ltl2ba(inner))),
        LtlFormula::Until(_l, _r) => Ltl2baFormula::False, // Not supported
        LtlFormula::Release(_l, _r) => Ltl2baFormula::False, // Not supported
    }
}
