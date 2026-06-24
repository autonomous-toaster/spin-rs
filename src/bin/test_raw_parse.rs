type Input<'a> = &'a str;

fn skip_ws(input: Input) -> Input {
    input.trim_start()
}

fn main() {
    let input = "active proctype P() { do :: x = 0 od }";
    println!("Input: '{}'", input);
    println!("After 'do': '{}'", &input[input.find("do").unwrap() + 2..]);

    // Simulate what parser sees after 'do'
    let after_do = " :: x = 0 od }";
    println!("After 'do' (with space): '{}'", after_do);
    println!("After skip_ws: '{}'", skip_ws(after_do));
}
