mod cli;
use clap::Parser;

fn main() {
    let cli = cli::Cli::parse();
    let rc = cli::run(cli);
    std::process::exit(rc);
}
