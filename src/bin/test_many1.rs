// Test: does many1 work with our guard_body?
// many1 expects at least one successful parse

// The issue might be that guard_body is failing, not that it's parsing wrong

fn main() {
    println!("many1(guard_body) requires at least one guard to parse successfully");
    println!("If guard_body fails on ':: x = 0', then do_stmt fails");
    println!();
    println!("Current guard_body logic:");
    println!("1. Check for 'else' - no");
    println!("2. Check if stmt-like (ident = ! ?) - YES for 'x = 0'");
    println!("3. If stmt-like, parse as body with no condition");
    println!("4. many0(stmt) should parse 'x = 0' as assignment_stmt");
    println!();
    println!("But it's returning 0 decls, so something is failing...");
}
