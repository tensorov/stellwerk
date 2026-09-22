//! CLI definitions — packet-03 (RED).

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
            ModuleCommand::Add(_) => {
                eprintln!("module add: not implemented (packet-05)");
                1
            }
            ModuleCommand::Measure(_) => {
                eprintln!("module measure: not implemented (packet-07a)");
                1
            }
            ModuleCommand::Audit(_) => {
                eprintln!("module audit: not implemented (packet-09)");
                1
            }
            ModuleCommand::Update(_) => {
                eprintln!("module update: not implemented (packet-11)");
                1
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
