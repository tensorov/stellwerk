use clap::Parser;
use stellwerk_task::cli::{self, Cli};

fn main() {
    std::process::exit(cli::run(Cli::parse()));
}
