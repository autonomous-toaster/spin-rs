// Debug: what does expr consume?
const TEST: &str = "x = 0";

fn main() {
    println!("Testing expr parser on '{}'", TEST);
    // Can't easily test just expr parser from bin
    // But we know it parses "x" and stops at "="
    println!("Expected: expr parser consumes 'x', leaves ' = 0'");
    println!("Our lookahead checks for '=' at start of remaining input");
    println!("So it should recognize this is an assignment statement");
}
