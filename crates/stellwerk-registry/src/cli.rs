//! CLI definitions — packet-03 (RED).

use std::path::Path;

use crate::cmd_add;
use crate::cmd_audit;
use crate::cmd_measure;
use crate::cmd_update;
use crate::model::ModulesFile;
use clap::{Parser, Subcommand, ValueEnum};

/// stellwerk — federation manager
#[derive(Debug, Parser)]
#[command(name = "stellwerk", about = "Stellwerk Federation CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Module {
        #[command(subcommand)]
        command: ModuleCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum ModuleCommand {
    Add(Box<AddArgs>),
    Measure(NameArgs),
    Audit(NameArgs),
    Update(NameArgs),
}

/// Kind argument — lowercased only.
#[derive(Debug, Clone, PartialEq, Eq, ValueEnum)]
pub enum KindArg {
    Mcp,
    Skill,
}

#[derive(Debug, PartialEq, Eq, Clone, clap::Args)]
pub struct AddArgs {
    pub name: String,

    #[arg(long)]
    pub kind: KindArg,

    #[arg(long)]
    pub git: Option<String>,

    #[arg(long)]
    pub rev: Option<String>,

    #[arg(long)]
    pub npm: Option<String>,

    #[arg(long, default_value_t = String::from("0.0.0"))]
    pub npm_version: String,

    #[arg(long)]
    pub cargo: Option<String>,

    #[arg(long, default_value_t = String::from("0.0.0"))]
    pub cargo_version: String,

    #[arg(long)]
    pub command: Option<String>,

    #[arg(long, value_delimiter = ',')]
    pub tools: Vec<String>,

    #[arg(long)]
    pub risk: Option<String>,

    #[arg(long)]
    pub no_autoload: bool,

    #[arg(long)]
    pub update_channel: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, clap::Args)]
pub struct NameArgs {
    pub name: Option<String>,
}

/// Parse command-line arguments.
pub fn parse_from<I, T>(args: I) -> Result<Cli, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    Cli::try_parse_from(args)
}

/// Dispatch CLI — run() with exit codes.
pub fn dispatch(cli: &Cli) -> i32 {
    match &cli.command {
        Command::Module { command } => match command {
            ModuleCommand::Add(a) => match cmd_add::run(a, &std::env::current_dir().unwrap()) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("{}", e);
                    1
                }
            },
            ModuleCommand::Measure(a) => {
                match cmd_measure::run(a, &std::env::current_dir().unwrap()) {
                    Ok(()) => 0,
                    Err(e) => {
                        eprintln!("{}", e);
                        1
                    }
                }
            }
            ModuleCommand::Update(a) => {
                match cmd_update::run(a, &std::env::current_dir().unwrap()) {
                    Ok(()) => 0,
                    Err(e) => {
                        eprintln!("{}", e);
                        1
                    }
                }
            }
            ModuleCommand::Audit(a) => {
                // Имя модуля и корень обязательны: неизвестный корень — отказ с кодом 1,
                // а не путь по умолчанию (тот же AGENTS.md, закон 6).
                let root = match std::env::current_dir() {
                    Ok(root) => root,
                    Err(e) => {
                        eprintln!("module audit: current directory is unavailable: {e}");
                        return 1;
                    }
                };
                match audit(a, &root, cmd_audit::AUDIT_TIMEOUT_MS) {
                    Ok((report, code)) => {
                        print!("{report}");
                        code
                    }
                    Err(e) => {
                        eprintln!("module audit: {e}");
                        1
                    }
                }
            }
        },
    }
}

/// Run the CLI, returning an exit code.
pub fn run(args: Vec<std::ffi::OsString>) -> i32 {
    match parse_from(args) {
        Err(e) => {
            let _ = e.print();
            match e.kind() {
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => 0,
                _ => 2,
            }
        }
        Ok(cli) => dispatch(&cli),
    }
}

/// Отказ `module audit` на уровне CLI: имя обязательно, модуль должен быть в манифесте,
/// манифест должен читаться. Ни один из них не паника и не «пустой OK».
#[derive(Debug, thiserror::Error)]
pub enum CliAuditError {
    #[error("module required: stellwerk module audit <name>")]
    NameRequired,
    #[error("no such module: {0} (not declared in modules.toml)")]
    NoModule(String),
    #[error("modules.toml: {0}")]
    Manifest(#[from] crate::model::ModelError),
    #[error(transparent)]
    Audit(#[from] cmd_audit::AuditError),
}

/// Аудит одного модуля: вернуть `(отчёт, код возврата)`.
///
/// `timeout_ms` инжектится, а не берётся из константы: прод-дефолт в 10 секунд слишком
/// долог для теста, а зашитый в тело дефолт тихо утопил бы тестовый бюджет (packet-08).
pub fn audit(
    args: &NameArgs,
    root: &Path,
    timeout_ms: u64,
) -> Result<(String, i32), CliAuditError> {
    let name = args.name.as_deref().ok_or(CliAuditError::NameRequired)?;
    let mf = ModulesFile::load(&root.join("modules.toml"))?;
    let module = mf
        .module
        .get(name)
        .ok_or_else(|| CliAuditError::NoModule(name.to_owned()))?;
    let report = cmd_audit::audit_module(module, root, timeout_ms)?;
    let clean = report.missing_in_server.is_empty()
        && report.extra_in_server.is_empty()
        && report.not_invocable.is_empty();
    Ok((audit_report_text(name, &report), i32::from(!clean)))
}

/// Отчёт человекочитаемый: по строке на каждое расхождение, `OK` при полном совпадении.
///
/// `skill_skipped` — отдельная строка вместо `OK`: «аудит не выполнялся» и «аудит прошёл»
/// обязаны выглядеть по-разному, иначе отложенная проверка читается как успешная.
fn audit_report_text(name: &str, report: &cmd_audit::AuditReport) -> String {
    if report.skill_skipped {
        return format!("audit {name}: SKIPPED — kind = skill, the server was not spawned\n");
    }
    let mut lines = Vec::new();
    for (label, entries) in [
        ("missing_in_server", &report.missing_in_server),
        ("extra_in_server", &report.extra_in_server),
        ("not_invocable", &report.not_invocable),
    ] {
        lines.extend(entries.iter().map(|entry| format!("  {label}: {entry}")));
    }
    if lines.is_empty() {
        return format!("audit {name}: OK\n");
    }
    format!("audit {name}:\n{}\n", lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_add_mcp_git() {
        let cli = parse_from([
            "stellwerk",
            "module",
            "add",
            "polyforge",
            "--kind",
            "mcp",
            "--git",
            "https://example.com/x",
            "--rev",
            "abc123",
            "--command",
            "polyforge-mcp",
            "--tools",
            "a,b",
            "--risk",
            "R3",
            "--update-channel",
            "tag",
        ]);
        let c = cli.unwrap();
        match &c.command {
            Command::Module { command } => match command {
                ModuleCommand::Add(a) => {
                    assert_eq!(a.name, "polyforge");
                    assert_eq!(a.kind, KindArg::Mcp);
                    assert_eq!(a.git.as_deref(), Some("https://example.com/x"));
                    assert_eq!(a.rev.as_deref(), Some("abc123"));
                    assert_eq!(a.command.as_deref(), Some("polyforge-mcp"));
                    assert_eq!(a.tools, vec!["a", "b"]);
                    assert_eq!(a.risk.as_deref(), Some("R3"));
                    assert_eq!(a.update_channel.as_deref(), Some("tag"));
                    assert!(!a.no_autoload);
                }
                _ => panic!("expected Add"),
            },
        }
    }

    #[test]
    fn parse_add_skill_npm() {
        let cli = parse_from([
            "stellwerk",
            "module",
            "add",
            "my-skill",
            "--kind",
            "skill",
            "--npm",
            "pkg",
            "--npm-version",
            "1.0",
        ]);
        let c = cli.unwrap();
        match &c.command {
            Command::Module { command } => match command {
                ModuleCommand::Add(a) => {
                    assert_eq!(a.name, "my-skill");
                    assert_eq!(a.kind, KindArg::Skill);
                    assert_eq!(a.npm.as_deref(), Some("pkg"));
                    assert_eq!(a.npm_version, "1.0");
                }
                _ => panic!("expected Add"),
            },
        }
    }

    #[test]
    fn parse_add_mcp_cargo() {
        let cli = parse_from([
            "stellwerk",
            "module",
            "add",
            "crate-module",
            "--kind",
            "mcp",
            "--cargo",
            "some-crate",
            "--cargo-version",
            "2.0",
        ]);
        let c = cli.unwrap();
        match &c.command {
            Command::Module { command } => match command {
                ModuleCommand::Add(a) => {
                    assert_eq!(a.name, "crate-module");
                    assert_eq!(a.kind, KindArg::Mcp);
                    assert_eq!(a.cargo.as_deref(), Some("some-crate"));
                    assert_eq!(a.cargo_version, "2.0");
                }
                _ => panic!("expected Add"),
            },
        }
    }

    #[test]
    fn parse_add_no_autoload() {
        let cli = parse_from([
            "stellwerk",
            "module",
            "add",
            "skip-me",
            "--kind",
            "mcp",
            "--git",
            "https://example.com/x",
            "--no-autoload",
        ]);
        let c = cli.unwrap();
        match &c.command {
            Command::Module { command } => match command {
                ModuleCommand::Add(a) => {
                    assert!(a.no_autoload);
                }
                _ => panic!("expected Add"),
            },
        }
    }

    #[test]
    fn parse_measure_with_name() {
        let cli = parse_from(["stellwerk", "module", "measure", "polyforge"]);
        let c = cli.unwrap();
        match &c.command {
            Command::Module { command } => match command {
                ModuleCommand::Measure(a) => {
                    assert_eq!(a.name, Some("polyforge".to_string()));
                }
                _ => panic!("expected Measure"),
            },
        }
    }

    #[test]
    fn parse_measure_no_name() {
        let cli = parse_from(["stellwerk", "module", "measure"]);
        let c = cli.unwrap();
        match &c.command {
            Command::Module { command } => match command {
                ModuleCommand::Measure(a) => {
                    assert!(a.name.is_none());
                }
                _ => panic!("expected Measure"),
            },
        }
    }

    #[test]
    fn parse_audit() {
        let cli = parse_from(["stellwerk", "module", "audit", "x"]);
        let c = cli.unwrap();
        match &c.command {
            Command::Module { command } => match command {
                ModuleCommand::Audit(a) => {
                    assert_eq!(a.name, Some("x".to_string()));
                }
                _ => panic!("expected Audit"),
            },
        }
    }

    #[test]
    fn parse_update() {
        let cli = parse_from(["stellwerk", "module", "update"]);
        let c = cli.unwrap();
        match &c.command {
            Command::Module { command } => match command {
                ModuleCommand::Update(a) => {
                    assert!(a.name.is_none());
                }
                _ => panic!("expected Update"),
            },
        }
    }

    #[test]
    fn run_badcmd_exit2() {
        let code = run(vec!["stellwerk".into(), "badcmd".into()]);
        assert_eq!(code, 2);
    }

    #[test]
    fn run_module_no_subcmd_exit2() {
        let code = run(vec!["stellwerk".into(), "module".into()]);
        assert_eq!(code, 2);
    }

    #[test]
    fn run_help_exit0() {
        let code = run(vec!["stellwerk".into(), "--help".into()]);
        assert_eq!(code, 0);
    }

    #[test]
    fn run_module_add_placeholder_exit1() {
        let code = run(vec![
            "stellwerk".into(),
            "module".into(),
            "add".into(),
            "polyforge".into(),
            "--kind".into(),
            "mcp".into(),
            "--git".into(),
            "https://e/x".into(),
            "--rev".into(),
            "abc".into(),
        ]);
        assert_eq!(code, 1);
    }
}
