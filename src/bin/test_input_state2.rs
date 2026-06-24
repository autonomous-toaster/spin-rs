fn main() {
    let input = "x = 0 od }";
    println!("Input: '{}'", input);
    println!("After 'x': '{}'", &input[1..]);
    println!("Trimmed: '{}'", input[1..].trim_start());
    println!(
        "Starts with '=': {}",
        input[1..].trim_start().starts_with('=')
    );
}
