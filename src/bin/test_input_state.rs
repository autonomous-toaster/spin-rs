fn main() {
    let input = "x = 0 od }";
    println!("Input: '{}'", input);
    println!("Trimmed: '{}'", input.trim_start());
    println!("Starts with '=': {}", input.trim_start().starts_with('='));
}
