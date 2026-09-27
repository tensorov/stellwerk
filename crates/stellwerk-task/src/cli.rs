//! CLI-модуль. Реализация в packet-04.

use clap::{Parser, Subcommand};

// ═══════════════════════════════════════════════════════════════════════════
// CLI structure
// ═══════════════════════════════════════════════════════════════════════════

/// Stellwerk task orchestrator
#[derive(Debug, Parser)]
#[command(name = "stellwerk")]
pub struct Cli {
    #[command(subcommand)]
    pub task: TopLevelCommands,
}

/// Task operations
#[derive(Debug, Subcommand)]
pub enum TaskCommands {
    /// Apply a task specification from a TOML file
    Apply {
        /// Path to the TOML task file
        #[arg(long)]
        file: String,
    },
    /// Read task specification and status
    Get {
        /// Task identifier
        id: String,
    },
    /// Stream task events
    Watch {
        /// Task identifier
        id: String,
    },
    /// Stop a task (transition to Suspended)
    Cancel {
        /// Task identifier
        id: String,
    },
    /// Show run journal
    Logs {
        /// Task identifier
        id: String,
    },
}

/// Top-level command: `stellwerk task <action>`
#[derive(Debug, Subcommand)]
pub enum TopLevelCommands {
    /// Task operations
    Task {
        #[command(subcommand)]
        cmd: TaskCommands,
    },
}

// ═══════════════════════════════════════════════════════════════════════════
// Public API (stub — packet-04)
// ═══════════════════════════════════════════════════════════════════════════

/// Entry point for CLI execution.
///
/// Returns:
/// - 0 on success
/// - 1 on execution error
/// - 2 on argument parse error
pub fn run(_cli: Cli) -> i32 {
    // Stub: always returns 1 (execution error) — packet-04 will implement.
    1
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_parse_apply() {
        let cli = Cli::try_parse_from(["stellwerk", "task", "apply", "--file", "t.toml"])
            .expect("apply should parse");
        let TopLevelCommands::Task { cmd } = cli.task;
        let file = match cmd {
            TaskCommands::Apply { file } => file,
            _ => panic!("expected Apply"),
        };
        assert_eq!(file, "t.toml");
    }

    #[test]
    fn cli_parse_get() {
        let cli =
            Cli::try_parse_from(["stellwerk", "task", "get", "t-1"]).expect("get should parse");
        let TopLevelCommands::Task { cmd } = cli.task;
        let id = match cmd {
            TaskCommands::Get { id } => id,
            _ => panic!("expected Get"),
        };
        assert_eq!(id, "t-1");
    }

    #[test]
    fn cli_parse_watch() {
        let cli =
            Cli::try_parse_from(["stellwerk", "task", "watch", "t-1"]).expect("watch should parse");
        let TopLevelCommands::Task { cmd } = cli.task;
        let id = match cmd {
            TaskCommands::Watch { id } => id,
            _ => panic!("expected Watch"),
        };
        assert_eq!(id, "t-1");
    }

    #[test]
    fn cli_parse_cancel() {
        let cli = Cli::try_parse_from(["stellwerk", "task", "cancel", "t-1"])
            .expect("cancel should parse");
        let TopLevelCommands::Task { cmd } = cli.task;
        let id = match cmd {
            TaskCommands::Cancel { id } => id,
            _ => panic!("expected Cancel"),
        };
        assert_eq!(id, "t-1");
    }

    #[test]
    fn cli_parse_logs() {
        let cli =
            Cli::try_parse_from(["stellwerk", "task", "logs", "t-1"]).expect("logs should parse");
        let TopLevelCommands::Task { cmd } = cli.task;
        let id = match cmd {
            TaskCommands::Logs { id } => id,
            _ => panic!("expected Logs"),
        };
        assert_eq!(id, "t-1");
    }

    #[test]
    fn cli_parse_nonsense() {
        let result = Cli::try_parse_from(["stellwerk", "task", "nonsense"]);
        assert!(result.is_err(), "unknown subcommand should fail");
    }

    #[test]
    fn cli_parse_get_no_id() {
        let result = Cli::try_parse_from(["stellwerk", "task", "get"]);
        assert!(result.is_err(), "get without ID should fail");
    }

    #[test]
    fn cli_run_returns_1() {
        let cli = Cli::try_parse_from(["stellwerk", "task", "get", "t-1"]).unwrap();
        let rc = run(cli);
        // `t-1` не создаётся ни одним тестом, так что исход предопределён, и
        // тест не зависит от остатков в общей временной базе. Нужен 1, а не 0:
        // иначе опечатка в id неотличима от найденной задачи.
        assert_eq!(
            rc, 1,
            "get несуществующей задачи обязан вернуть 1 (packet-04, п.3)"
        );
    }
}
