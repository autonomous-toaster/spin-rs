fn main() {
    env_logger::init();

    let args = std::env::args().collect::<Vec<_>>();

    if args.len() < 2 {
        eprintln!("Usage: spin-rs [-a | -run | --help] <model.pml>");
        std::process::exit(1);
    }

    if let Err(e) = spin_rs::cli::run(&args[1..]) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
