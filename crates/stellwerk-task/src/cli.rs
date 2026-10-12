//! CLI-модуль. Реализация в packet-04.

use std::io::Write;
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use stellwerk_core::contract::Event;
use stellwerk_store::{EventStore, SqliteStore, StoreError, TaskStore};

use crate::taskfile::TaskFile;

// ═══════════════════════════════════════════════════════════════════════════
// CLI structure
// ═══════════════════════════════════════════════════════════════════════════

/// Stellwerk task orchestrator
#[derive(Debug, Parser)]
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
// Public API (packet-04)
// ═══════════════════════════════════════════════════════════════════════════

/// Entry point for CLI execution.
///
/// Returns:
/// - 0 on success
/// - 1 on execution error
/// - 2 on argument parse error
pub fn run(cli: Cli) -> i32 {
    let mut out = std::io::stdout();
    let mut err = std::io::stderr();
    let rc = run_with(cli, &mut out, &mut err);
    // `std::process::exit` не разрушает буферы: без сброса теряется последняя строка.
    let _ = (out.flush(), err.flush());
    rc
}

fn run_with(cli: Cli, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let TopLevelCommands::Task { cmd } = cli.task;
    let store = match SqliteStore::open(&db_path(std::env::var("STELLWERK_DB").ok())) {
        Ok(s) => s,
        Err(e) => return fail(err, &format!("error: {e}")),
    };
    // Мост односторонний: внутри `.await` живут только фьючи хранилища, ни одного
    // `MutexGuard` — держать его через await нельзя.
    let rt = match tokio::runtime::Builder::new_current_thread().build() {
        Ok(rt) => rt,
        Err(e) => return fail(err, &format!("error: runtime: {e}")),
    };
    rt.block_on(async move {
        match cmd {
            TaskCommands::Apply { file } => {
                let spec = match TaskFile::load(Path::new(&file)).and_then(TaskFile::into_spec) {
                    Ok(spec) => spec,
                    Err(e) => return fail(err, &format!("error: task file: {e}")),
                };
                match store.create(spec).await {
                    Ok(saved) => {
                        emit(out, &format!("applied {}", saved.id));
                        0
                    }
                    Err(StoreError::Conflict(msg)) => fail(err, &format!("error: conflict: {msg}")),
                    Err(e) => fail(err, &format!("error: store: {e}")),
                }
            }
            TaskCommands::Get { id } => match store.get(&id).await {
                Ok(None) => fail(err, &format!("error: not found: {id}")),
                Ok(Some(spec)) => {
                    emit(out, &format!("id: {}", spec.id));
                    emit(out, &format!("goal: {}", spec.goal));
                    emit(out, &format!("workspace_ref: {}", spec.workspace_ref));
                    0
                }
                Err(e) => fail(err, &format!("error: store: {e}")),
            },
            // Существование задачи спрашивается ТОТ ЖЕ выборкой, что у `get`:
            // `list_by_task` у несуществующей задачи вернул бы пустой вектор, и
            // «задачи нет» стало бы неотличимо от «задача есть, событий нет» —
            // ровно то смешение, которое запрещает закон 6.
            TaskCommands::Watch { id } => match store.get(&id).await {
                Ok(None) => fail(err, &format!("error: not found: {id}")),
                Ok(Some(_)) => watch_events(&store, &id, out, err).await,
                Err(e) => fail(err, &format!("error: store: {e}")),
            },
            TaskCommands::Cancel { .. } => stub("cancel", "N2+", err),
            TaskCommands::Logs { .. } => stub("logs", "N2+", err),
        }
    })
}

/// Печать потока событий задачи. Порядок — тот, что отдал трейт, ничего не
/// сортируется и не догоняется: второй порядок означал бы вторую правду о
/// журнале.
async fn watch_events(
    store: &SqliteStore,
    id: &str,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    match store.list_by_task(id).await {
        // Отсутствие событий у СУЩЕСТВУЮЩЕЙ задачи — факт, а не ошибка: код 0 и
        // строка на stdout. Код 1 здесь не поставил бы «событий нет» в один ряд с
        // «задачи нет» и выглядел бы поломкой на только что созданной задаче.
        Ok(events) if events.is_empty() => {
            emit(out, &format!("no events for {id}"));
            0
        }
        Ok(events) => {
            for event in &events {
                emit(out, &event_line(event));
            }
            0
        }
        Err(e) => fail(err, &format!("error: store: {e}")),
    }
}

/// Строка события: `<timestamp> <kind> <краткое описание>`.
///
/// Отдельного поля «описание» у `Event` нет, а выдумывать содержимое нельзя
/// (пакет, п.2): краткое описание — это `actor`, «кто совершил действие», взятое
/// из события дословно. `payload` на это не годится — он пуст у события без
/// содержимого, и строка с хвостовым пробелом была бы неотличима от события, у
/// которого описания нет вовсе.
fn event_line(event: &Event) -> String {
    format!("{} {} {}", event.ts, event.event_type, event.actor)
}

/// Код 1 и имя пакета, который реализует заглушку: молчаливый отказ запрещён
/// законом 6 AGENTS.md — «не реализовано» неотличимо от «отказано».
fn stub(command: &str, packet: &str, err: &mut dyn Write) -> i32 {
    emit(err, &format!("not implemented: {command} ({packet})"));
    1
}

fn emit(w: &mut dyn Write, line: &str) {
    let _ = writeln!(w, "{line}");
}

fn fail(err: &mut dyn Write, message: &str) -> i32 {
    emit(err, message);
    1
}

/// Отсутствие `STELLWERK_DB` и его поломка — разные состояния: отсутствие даёт
/// временную базу (п.1), поломка обязана упасть при открытии.
fn db_path(env_value: Option<String>) -> PathBuf {
    match env_value {
        Some(p) => PathBuf::from(p),
        None => std::env::temp_dir().join("stellwerk-task.db"),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

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

    // База по умолчанию — файл в temp_dir, и он ОДИН на весь модуль: тесты берут
    // лок, иначе параллельные писатели ловят SQLITE_BUSY (у rusqlite busy_timeout = 0).
    static DB_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn unique_id(tag: &str) -> String {
        format!("t-cli-{tag}-{}", std::process::id())
    }

    fn write_task_file(tag: &str, id: &str) -> PathBuf {
        let body = format!(
            "[task]\n\
             id = \"{id}\"\n\
             goal = \"Проверить dispatch CLI\"\n\
             workspace_ref = \"ws-cli\"\n\
             model_ref = \"m-default\"\n\
             packet = \"M2/packet-04\"\n\
             limits = {{ max_files = 3, max_diff_lines = 50, timeout_seconds = 60,\n\
             max_retries = 1, context_budget_tokens = 1000, data_class = \"internal\" }}\n"
        );
        let path = std::env::temp_dir().join(format!("sw-{tag}-{}.toml", std::process::id()));
        fs::write(&path, body).expect("task file must be writable");
        path
    }

    fn run_capture(args: &[&str]) -> (i32, String, String) {
        let mut argv = vec!["stellwerk", "task"];
        argv.extend_from_slice(args);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let parsed = Cli::try_parse_from(argv).expect("command must parse");
        let rc = run_with(parsed, &mut out, &mut err);
        let text = |b: Vec<u8>| String::from_utf8_lossy(&b).into();
        (rc, text(out), text(err))
    }

    #[test]
    fn cli_apply_creates_task_and_reports_conflict() {
        let _guard = DB_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let id = unique_id("apply");
        let first = write_task_file("apply-1", &id);
        let second = write_task_file("apply-2", &id);
        let (rc, out, err) = run_capture(&["apply", "--file", &first.to_string_lossy()]);
        assert_eq!(rc, 0, "apply обязан выйти 0; stderr: {err}");
        assert_eq!(out, format!("applied {id}\n"), "нет `applied <id>`: {out}");
        let (rc, out, err) = run_capture(&["apply", "--file", &second.to_string_lossy()]);
        let _ = (fs::remove_file(&first), fs::remove_file(&second));
        assert_eq!(rc, 1, "apply с повторным id обязан вернуть 1 (п.2)");
        assert_eq!(out, "", "конфликт не должен печатать в stdout");
        assert!(err.contains("conflict"), "нет `conflict` в stderr: {err}");
    }

    #[test]
    fn cli_get_prints_spec_and_reports_absence() {
        let _guard = DB_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let id = unique_id("get");
        let path = write_task_file("get", &id);
        let (rc, _, err) = run_capture(&["apply", "--file", &path.to_string_lossy()]);
        assert_eq!(rc, 0, "apply перед get обязан удаться; stderr: {err}");
        let (rc, out, err) = run_capture(&["get", &id]);
        let (gone, _, gone_err) = run_capture(&["get", &unique_id("absent")]);
        let _ = fs::remove_file(&path);
        assert_eq!(rc, 0, "get существующей задачи обязан вернуть 0; {err}");
        assert!(out.contains(&format!("id: {id}")), "нет id: {out}");
        assert!(
            out.contains("goal: Проверить dispatch CLI"),
            "нет goal: {out}"
        );
        assert!(
            out.contains("workspace_ref: ws-cli"),
            "нет workspace_ref: {out}"
        );
        assert_eq!(gone, 1, "get несуществующей задачи обязан вернуть 1");
        assert!(
            gone_err.contains("not found"),
            "нет `not found`: {gone_err}"
        );
    }

    #[test]
    fn cli_stubs_name_their_packet() {
        let _guard = DB_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // `watch` из таблицы убран M4/packet-04 (п.4 — «убрать заглушку `watch`
        // из `run`»): его контракт «вернуть 1 с текстом not implemented» пакет
        // заменил на чтение событий, и проверка этого контракта живёт теперь в
        // `tests/cli_watch.rs` тремя случаями вместо одной заглушки. `cancel` и
        // `logs` остаются заглушками — их утверждения не тронуты.
        for (command, packet) in [("cancel", "N2+"), ("logs", "N2+")] {
            let (rc, out, err) = run_capture(&[command, "t-1"]);
            assert_eq!(rc, 1, "{command} обязан вернуть 1 (п.4)");
            assert_eq!(out, "", "{command} не должен печатать в stdout");
            let expected = format!("not implemented: {command} ({packet})");
            assert!(
                err.contains(&expected),
                "{command}: нет `{expected}`: {err}"
            );
        }
    }

    #[test]
    fn cli_db_path_prefers_env_over_temp_dir() {
        let over = PathBuf::from("/tmp/stellwerk-override.db");
        assert_eq!(db_path(Some(over.display().to_string())), over);
        assert_eq!(
            db_path(None),
            std::env::temp_dir().join("stellwerk-task.db")
        );
    }
}
