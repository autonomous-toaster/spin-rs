// The issue: when we parse "x = 0" in a guard,
// expr parser consumes "x" and stops at "="
// Then we check for "->" and don't find it
// So we treat "x" as a condition with no arrow
// But we should recognize that "=" means this is an assignment statement

// Solution: after parsing expr, if no arrow, check if next token is
// "=", "!", "?" which would indicate a statement, not a condition
// If so, re-parse the whole thing as a statement

fn main() {
    println!("Need to check for statement-starting tokens after expr");
    println!("Tokens that indicate statement: =, !, ?");
}
