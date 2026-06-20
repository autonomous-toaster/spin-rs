fn main() {
    env_logger::init();

    let args = std::env::args().collect::<Vec<_>>();

    if let Err(e) = spin_rs::cli::run(&args) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
