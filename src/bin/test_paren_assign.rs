// Test if assignment in parens should fail
fn main() {
    println!("In Promela, assignments are statements, not expressions.");
    println!("So (x = 0) should NOT parse as an expression.");
    println!("But x = 0 should parse as a statement.");
    println!();
    println!("The issue is that expr parser has:");
    println!("  delimited(ws_char('('), expr, ws_char(')'))");
    println!("which tries to parse (x = 0) as expr, but x = 0 is not an expr.");
}
