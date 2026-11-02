//! N1-01: чек обязан ЛЕЧЬ в SQLite и читаться обратно, а провал запуска — не
//! превращаться в `verdict=Allow`.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use stellwerk_cli::build_runner::{run_build, BuildRun};
use stellwerk_cli::plan::BuildRequest;
use stellwerk_store::{EventStore, SqliteStore};
use stellwerk_task::m5::model_transport::{BackendError, Frame, ModelBackend, TurnMessage};
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

/// Свежий каталог на тест: тесты идут параллельно, pid+seq уникален.
fn scratch_dir(name: &str) -> PathBuf {
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let seq = SEQ.fetch_add(1, Ordering::SeqCst);
    let dir =
        std::env::temp_dir().join(format!("stellwerk-r04-{name}-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// Подставной Pi: берём ту же fixture-программу, что лежит в `examples`, но
/// запускаем её как ПРОГРАММУ (`PI_BIN`), а не ищем рядом с task-файлом.
fn fake_pi(dir: &Path, marker: &str) -> PathBuf {
    let fixture = workspace_root().join("crates/stellwerk-cli/examples/pi-fixture.sh");
    let source = std::fs::read_to_string(&fixture).expect("read example fixture");
    let body = source.replacen("#!/bin/sh\n", &format!("#!/bin/sh\necho {marker} >&2\n"), 1);
    let pi = dir.join("pi");
    std::fs::write(&pi, body).expect("write fake pi");
    let mut mode = std::fs::metadata(&pi).expect("pi metadata").permissions();
    mode.set_mode(0o755);
    std::fs::set_permissions(&pi, mode).expect("pi executable");
    pi
}

/// Пробник вместо Pi: успешный `stderr` прогона не печатается, поэтому факт
/// запуска доказуем отказом с маркером, а не нулевым кодом.
fn probe_pi(dir: &Path, marker: &str) -> PathBuf {
    let pi = dir.join("pi-probe");
    std::fs::write(&pi, format!("#!/bin/sh\necho {marker} >&2\nexit 42\n"))
        .expect("write pi probe");
    let mut mode = std::fs::metadata(&pi)
        .expect("probe metadata")
        .permissions();
    mode.set_mode(0o755);
    std::fs::set_permissions(&pi, mode).expect("probe executable");
    pi
}

/// Путь к транспорту Pi. Без него прогон отказывает названно, а не стартует
/// движок без канала модели.
fn pi_extension() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../stellwerk-task/pi/stellwerk_transport.ts")
}

fn run_cli_with(task_path: &Path, pi: Option<&Path>, home: &Path) -> RunResult {
    let mut command = Command::new(env!("CARGO_BIN_EXE_stellwerk"));
    command
        .current_dir(workspace_root())
        .arg("run")
        .arg(task_path);
    // Изоляция от машины разработчика: отсутствие Pi проверяется на пустом HOME,
    // а не на установленном у владельца.
    command.env_remove("PI_BIN");
    command.env_remove("STELLWERK_PI_EXTENSION");
    command.env("HOME", home);
    if let Some(pi) = pi {
        command.env("PI_BIN", pi);
        command.env("STELLWERK_PI_EXTENSION", pi_extension());
    }
    let output = command.output().expect("cli binary should spawn");
    RunResult {
        status: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// Успешные прогоны получают явный `PI_BIN`; production его больше не ищет.
fn run_cli(task_path: &Path) -> RunResult {
    let dir = scratch_dir("run-cli");
    let pi = fake_pi(&dir, "FAKE_PI_INVOKED");
    run_cli_with(task_path, Some(&pi), &dir)
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

/// Провал запуска обязан быть провалом: ненулевой код выхода, НЕТ `verdict=`.
///
/// Фикстура лежит РЯДОМ с файлом задачи, и задача в пустом каталоге запускаться
/// не может: `pi-fixture.sh` рядом нет — это ровно тот случай, который раньше
/// «успешно» превращался в `verdict=Allow` с пустым выводом.
#[test]
fn failed_run_is_not_reported_as_success() {
    let dir = scratch_dir("no-engine");
    let home = dir.join("home");
    std::fs::create_dir_all(&home).expect("create home");
    let task_path = dir.join("no-engine.toml");
    let toml = "[task]\nid = \"n1-fail\"\ngoal = \"echo hello\"\nworkspace_ref = \"ws\"\nmodel_ref = \"m\"\npacket = \"N1-10\"\nprofile = \"pi\"\nevidence_policy = \"e\"\n\n[task.limits]\nmax_files = 1\nmax_diff_lines = 1\ntimeout_seconds = 30\nmax_retries = 0\ncontext_budget_tokens = 1\ndata_class = \"internal\"\n";
    std::fs::write(&task_path, toml).expect("write fixture");

    let result = run_cli_with(&task_path, None, &home);
    assert_ne!(
        result.status,
        Some(0),
        "a run without an engine must fail; stdout: {}",
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
    assert!(
        result.stderr.contains("MissingEngine"),
        "stderr must name the missing engine: {}",
        result.stderr
    );
    assert!(
        !result.stderr.contains("fixture"),
        "absence must not be described as a fixture: {}",
        result.stderr
    );

    let _ = std::fs::remove_file(&task_path);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Отсутствие Pi — названный отказ, а не тихий переход на фикстуру. Присутствие
/// запускает именно `PI_BIN`, и это видно по маркеру из самого скрипта.
#[test]
fn missing_engine_is_named_refusal() {
    let dir = scratch_dir("missing-engine");
    let home = dir.join("home");
    std::fs::create_dir_all(&home).expect("create home");
    let task_path = dir.join("task.toml");
    let toml = "[task]\nid = \"r04-engine\"\ngoal = \"echo hello\"\nworkspace_ref = \"ws\"\nmodel_ref = \"m\"\npacket = \"V1/r04\"\nprofile = \"pi\"\nevidence_policy = \"e\"\n\n[task.limits]\nmax_files = 1\nmax_diff_lines = 1\ntimeout_seconds = 30\nmax_retries = 0\ncontext_budget_tokens = 1\ndata_class = \"internal\"\n";
    std::fs::write(&task_path, toml).expect("write task");

    let absent = run_cli_with(&task_path, None, &home);
    assert_ne!(absent.status, Some(0), "stdout: {}", absent.stdout);
    assert!(
        !absent.stdout.contains("verdict="),
        "stdout: {}",
        absent.stdout
    );
    assert!(
        absent.stderr.contains("MissingEngine"),
        "absence must be a named MissingEngine refusal: {}",
        absent.stderr
    );
    assert!(
        !absent.stderr.contains("pi-fixture.sh"),
        "no fixture fallback may be named: {}",
        absent.stderr
    );

    let pi = probe_pi(&dir, "FAKE_PI_INVOKED");
    let present = run_cli_with(&task_path, Some(&pi), &home);
    assert_ne!(present.status, Some(0), "probe must fail after speaking");
    assert!(
        present.stderr.contains("FAKE_PI_INVOKED"),
        "the PI_BIN program must be the engine that ran: {}",
        present.stderr
    );
    assert!(
        !present.stderr.contains("pi-fixture.sh"),
        "run must not name the fixture script: {}",
        present.stderr
    );
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

/// Build-путь проверяется по чеку авторизации, а не по ответу модели: пустой ход —
/// это факт «агент ничего не попросил», а не отказ.
struct SilentModel;

impl ModelBackend for SilentModel {
    fn turn(&self, _messages: &[TurnMessage]) -> Result<Vec<Frame>, BackendError> {
        Ok(Vec::new())
    }
}

/// Подставной `pi`: тест доказывает общий путь авторизации, а не работу агента,
/// поэтому процессу достаточно завершиться успешно. Скрипт кладётся туда же, откуда
/// `run_build` его запускает, — путь даёт вызывающий, а не модуль.
fn silent_pi(dir: &Path) -> PathBuf {
    let agent = dir.join("agent");
    std::fs::create_dir_all(agent.join("bin")).expect("pi bin");
    std::fs::create_dir_all(agent.join("install")).expect("pi install");
    let pi = agent.join("bin/pi");
    std::fs::write(&pi, "#!/bin/sh\nexit 0\n").expect("write silent pi");
    let mut mode = std::fs::metadata(&pi).expect("pi metadata").permissions();
    mode.set_mode(0o755);
    std::fs::set_permissions(&pi, mode).expect("pi executable");
    pi
}

/// Один путь исполнения: Run и Plan-Build приходят в один `authorize`, и это видно по
/// чеку `run.call`, который пишет только `run_once`. До сведения путей Build исполнял
/// `worker.exec` напрямую и не оставлял ни решения, ни чека.
#[test]
fn both_entrypoints_share_one_authorization() {
    let fixture = workspace_root().join("crates/stellwerk-cli/examples/n1-smoke.toml");
    let result = run_cli(&fixture);
    assert_eq!(result.status, Some(0), "stderr: {}", result.stderr);
    let run_store = field(&result.stdout, "store")
        .expect("run prints its store")
        .to_string();
    let (_, run_events) = receipt_event(&run_store, "n1-smoke");
    let run_row = run_events
        .iter()
        .find(|event| event.event_type == "run.call")
        .expect("run entrypoint writes a run.call receipt");

    // 2. Build-путь — тот же прогон, но без task-файла: идентичность даёт дерево.
    let root = std::env::temp_dir().join(format!("stellwerk-r01-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let repo = root.join("repo");
    std::fs::create_dir_all(repo.join(".git")).expect(".git");
    std::fs::write(repo.join(".git/HEAD"), "ref: refs/heads/main\n").expect("HEAD");
    std::fs::create_dir_all(repo.join("src")).expect("src");
    std::fs::write(repo.join("src/lib.rs"), "pub fn one() -> u8 { 1 }\n").expect("lib.rs");
    let pi = silent_pi(&root);
    let request = BuildRequest {
        goal: "добавить строку".to_string(),
        allowed_changes: vec!["src/lib.rs".to_string()],
        acceptance: vec!["тест проходит".to_string()],
        checks: vec!["cargo test".to_string()],
    };
    let extension =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../stellwerk-task/pi/stellwerk_transport.ts");
    let build_root = root.join("run");
    let outcome = run_build(
        BuildRun {
            request: &request,
            repo: &repo,
            root: &build_root,
            extension: &extension,
            pi: &pi,
        },
        &SilentModel,
    )
    .expect("build run");

    let build_store = build_root.join("receipts.db");
    assert!(
        build_store.is_file(),
        "Build прошёл без авторизации: чек `{}` не записан",
        build_store.display()
    );
    let store = SqliteStore::open(&build_store).expect("open build store");
    let runtime = Runtime::new().expect("tokio runtime");
    let build_events = runtime
        .block_on(store.list_since("1970-01-01T00:00:00Z"))
        .expect("list build receipts");
    let build_row = build_events
        .iter()
        .find(|event| event.event_type == "run.call")
        .expect("build entrypoint writes a run.call receipt");

    let expected_id = format!("build:{}", repo.display());
    assert_eq!(
        build_row.payload.get("task_id").map(String::as_str),
        Some(expected_id.as_str()),
        "идентичность Build выводится из дерева"
    );
    assert_eq!(
        build_row.payload.get("caller"),
        run_row.payload.get("caller"),
        "оба входа спрашивают один и тот же слой разрешений"
    );
    assert_eq!(
        build_row.payload.get("interactive"),
        run_row.payload.get("interactive"),
        "оба входа неинтерактивны"
    );
    assert_eq!(
        build_row.payload.get("risk").map(String::as_str),
        Some("R0"),
        "риск Build берётся из BUILD_REGISTRY, а не из Call.risk"
    );
    assert_eq!(
        run_row.payload.get("risk").map(String::as_str),
        Some("R0"),
        "риск Run берётся из реестра воркера"
    );
    assert_eq!(
        build_row.payload.get("status").map(String::as_str),
        Some("Succeeded"),
        "Build прошёл authorize и исполнение: {:?}",
        build_row.payload
    );
    assert!(
        outcome.model_turns == 0 && outcome.gate_requests == 0,
        "тихий агент ничего не просил: turns={} gate={}",
        outcome.model_turns,
        outcome.gate_requests
    );

    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_file(&run_store);
}
