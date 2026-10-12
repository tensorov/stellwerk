//! N1-01: чек обязан ЛЕЧЬ в SQLite и читаться обратно, а провал запуска — не
//! превращаться в `verdict=Allow`.

use std::path::{Path, PathBuf};
use std::process::Command;

use stellwerk_store::{EventStore, SqliteStore};
use tokio::runtime::Runtime;

/// Корень workspace: `CARGO_MANIFEST_DIR` = `<root>/crates/stellwerk-cli`.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate lives two levels below the workspace root")
        .to_path_buf()
}

struct RunResult {
    status: Option<i32>,
    stdout: String,
    stderr: String,
}

fn run_cli(task_path: &Path) -> RunResult {
    let output = Command::new(env!("CARGO_BIN_EXE_stellwerk-cli"))
        .current_dir(workspace_root())
        .arg("run")
        .arg(task_path)
        .output()
        .expect("cli binary should spawn");
    RunResult {
        status: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// Значение поля `key=value` из строки вывода.
fn field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let prefix = format!("{key}=");
    text.split_whitespace()
        .find_map(|token| token.strip_prefix(prefix.as_str()))
}

fn receipt_event(
    store_path: &str,
    task_id: &str,
) -> (SqliteStore, Vec<stellwerk_core::contract::Event>) {
    let store = SqliteStore::open(Path::new(store_path)).expect("open store");
    let runtime = Runtime::new().expect("tokio runtime");
    let events = runtime
        .block_on(store.list_by_task(task_id))
        .expect("list receipt events");
    (store, events)
}

/// Успешный прогон: чек реально записан в SQLite и читается обратно по своему id.
#[test]
fn run_writes_receipt_to_sqlite() {
    let fixture = workspace_root().join("crates/stellwerk-cli/examples/n1-smoke.toml");
    let result = run_cli(&fixture);
    assert_eq!(result.status, Some(0), "stderr: {}", result.stderr);
    assert_eq!(
        field(&result.stdout, "verdict"),
        Some("Allow"),
        "stdout: {}",
        result.stdout
    );

    let receipt = field(&result.stdout, "receipt")
        .expect("receipt id printed")
        .to_string();
    let store_path = field(&result.stdout, "store")
        .expect("store path printed")
        .to_string();

    let (_store, events) = receipt_event(&store_path, "n1-smoke");
    let row = events
        .iter()
        .find(|event| event.id == receipt)
        .expect("receipt id must exist as a row in SQLite");
    assert_eq!(row.event_type, "run.call");
    assert_eq!(
        row.payload.get("status").map(String::as_str),
        Some("Succeeded")
    );
    assert_eq!(
        row.payload.get("task_id").map(String::as_str),
        Some("n1-smoke")
    );

    let _ = std::fs::remove_file(&store_path);
}

/// Провал запуска обязан быть провалом: ненулевой код выхода, НЕТ `verdict=`, а чек
/// всё равно записан со статусом `Failed`.
///
/// Фикстура просит запустить команду, которую нельзя запустить: `goal` длиннее
/// `MAX_ARG_STRLEN` (128 KiB), поэтому `execve` вернёт `E2BIG`. Это ровно тот случай,
/// который раньше «успешно» превращался в `verdict=Allow` с пустым выводом.
#[test]
fn failed_run_is_not_reported_as_success() {
    let dir = std::env::temp_dir().join(format!("stellwerk-n1-failtest-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    let task_path = dir.join("oversized-goal.toml");
    let huge = "x".repeat(200_000);
    let toml = format!(
        "[task]\nid = \"n1-fail\"\ngoal = \"{huge}\"\nworkspace_ref = \"ws\"\nmodel_ref = \"m\"\npacket = \"N1-01\"\nevidence_policy = \"e\"\n\n[task.limits]\nmax_files = 1\nmax_diff_lines = 1\ntimeout_seconds = 30\nmax_retries = 0\ncontext_budget_tokens = 1\ndata_class = \"internal\"\n"
    );
    std::fs::write(&task_path, toml).expect("write fixture");

    let result = run_cli(&task_path);
    assert_ne!(
        result.status,
        Some(0),
        "a run that cannot start must fail; stdout: {}",
        result.stdout
    );
    assert!(
        !result.stdout.contains("verdict="),
        "stdout must not claim a verdict: {}",
        result.stdout
    );
    assert!(
        result.stderr.contains("error:"),
        "stderr: {}",
        result.stderr
    );

    let store_path = field(&result.stderr, "store")
        .or_else(|| field(&result.stdout, "store"))
        .expect("store path printed")
        .to_string();
    let (_store, events) = receipt_event(&store_path, "n1-fail");
    let row = events
        .iter()
        .find(|event| event.event_type == "run.call")
        .expect("a failed run must still leave a receipt");
    assert_eq!(
        row.payload.get("status").map(String::as_str),
        Some("Failed")
    );

    let _ = std::fs::remove_file(&task_path);
    let _ = std::fs::remove_file(&store_path);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Отсутствующий файл задачи — операционная ошибка, ненулевой код и никакого вердикта.
#[test]
fn missing_task_file_exits_nonzero() {
    let result = run_cli(Path::new(
        "crates/stellwerk-cli/examples/does-not-exist.toml",
    ));
    assert_ne!(result.status, Some(0));
    assert!(!result.stdout.contains("verdict="));
    assert!(
        result.stderr.contains("task file"),
        "stderr: {}",
        result.stderr
    );
}
