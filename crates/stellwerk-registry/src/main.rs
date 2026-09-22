fn main() {
    let code = stellwerk_registry::cli::run(std::env::args_os().collect());
    std::process::exit(code);
}
