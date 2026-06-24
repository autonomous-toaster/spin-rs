fn main() {
    let input = " x = 0 od }";
    let trimmed = input.trim_start();
    println!("Input: '{}'", input);
    println!("Trimmed: '{}'", trimmed);

    let mut chars = trimmed.chars().peekable();
    let mut ident_chars = String::new();

    while let Some(&ch) = chars.peek() {
        if ch.is_alphanumeric() || ch == '_' {
            ident_chars.push(chars.next().unwrap());
        } else {
            break;
        }
    }

    println!("Ident chars: '{}'", ident_chars);
    println!("Next char: {:?}", chars.peek());

    if !ident_chars.is_empty()
        && let Some(&next_ch) = chars.peek()
    {
        println!("Next char is: '{}'", next_ch);
        println!(
            "Is statement operator: {}",
            matches!(next_ch, '=' | '!' | '?')
        );
    }
}
