//! `stellwerk-cli` — единый вход в федерацию: `run <task.toml>` и `module ...`.
//!
//! N1-10: `run` собирает всю цепочку N1 на fixture-агенте: вердикт → receipt →
//! Pi-`RunOutcome` → evidence-цепочка → статус «ждёт человека». `module`
//! делегирует в `stellwerk_registry` — логика команд не дублируется.

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use stellwerk_cli::{plan, run_task, tui};
use stellwerk_task::cli::TuiArgs;

#[derive(Debug, Parser)]
#[command(name = "stellwerk")]
struct Cli {
    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Run a task from a task.toml file
    Run {
        /// Path to the task.toml file
        path: PathBuf,
    },
    /// Manage federation modules (delegated to stellwerk-registry)
    Module {
        /// Arguments forwarded to the registry CLI
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<std::ffi::OsString>,
    },
    /// Open the terminal UI for a task file (TUI-02)
    Tui(TuiArgs),
    /// Plan a change in a repository: goal -> model dialogue -> explicit approval
    Plan {
        /// Path to the repository to plan changes in
        repo: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();
    if let Err(err) = run_main(&cli) {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run_main(cli: &Cli) -> Result<(), String> {
    match &cli.cmd {
        Commands::Run { path } => {
            let result = run_task(path.as_path())?;
            println!(
                "verdict={} receipt={} store={} risk={} gate={}",
                result.verdict,
                result.receipt_id,
                result.store_path.display(),
                result.risk,
                result.gate
            );
        }
        Commands::Module { args } => {
            let mut argv = vec![
                std::ffi::OsString::from("stellwerk"),
                std::ffi::OsString::from("module"),
            ];
            argv.extend(args.clone());
            std::process::exit(stellwerk_registry::cli::run(argv));
        }
        Commands::Tui(args) => {
            tui::run_tui(&args.path)?;
        }
        Commands::Plan { repo } => {
            plan::run_plan_tui(repo)?;
        }
    }
    Ok(())
}
