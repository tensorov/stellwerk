//! N1-01: чек обязан ЛЕЧЬ в SQLite и читаться обратно, а провал запуска — не
//! превращаться в `verdict=Allow`.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use stellwerk_cli::build_runner::{run_build, BuildRun};
use stellwerk_cli::host_model::ModelConfig;
use stellwerk_cli::plan::{BuildRequest, HOST_MODEL_ID};
use stellwerk_cli::run_task_with;
use stellwerk_store::{EventStore, SqliteStore};
use stellwerk_task::evidence::{EvidenceKind, EvidenceLedger};
use stellwerk_task::ledger_sqlite::SqliteEvidenceLedger;
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
    run_cli_with_cwd(task_path, pi, home, &workspace_root())
}

/// Тот же запуск, но рабочий каталог задаёт тест: привязка к коммиту выводится
/// из cwd, и отказ без `.git` проверяется только из чужого каталога.
///
/// Токен кладётся в `HOME/.config/stellwerk`: `ModelConfig::from_env` ищет его
/// там, а `XDG_CONFIG_HOME` снят, чтобы машина разработчика не подсунула свой.
fn run_cli_with_cwd(task_path: &Path, pi: Option<&Path>, home: &Path, cwd: &Path) -> RunResult {
    let config = home.join(".config/stellwerk");
    std::fs::create_dir_all(&config).expect("create agent config dir");
    std::fs::write(config.join("neuraldeep.token"), "test-token\n").expect("write dummy token");
    let mut command = Command::new(env!("CARGO_BIN_EXE_stellwerk"));
    command.current_dir(cwd).arg("run").arg(task_path);
    // Изоляция от машины разработчика: отсутствие Pi проверяется на пустом HOME,
    // а не на установленном у владельца.
    command.env_remove("PI_BIN");
    command.env_remove("STELLWERK_PI_EXTENSION");
    command.env_remove("XDG_CONFIG_HOME");
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

/// Успешное удостоверение обязано нести все три привязки: рецепт, политику и
/// коммит. `None` в любом из них — это удостоверение без привязки, а не успех.
#[test]
fn run_attestation_carries_bindings() {
    let fixture = workspace_root().join("crates/stellwerk-cli/examples/n1-smoke.toml");
    let result = run_cli(&fixture);
    assert_eq!(result.status, Some(0), "stderr: {}", result.stderr);
    let ledger_path = field(&result.stdout, "ledger")
        .expect("run prints its ledger path")
        .to_string();
    let ledger = SqliteEvidenceLedger::open(Path::new(&ledger_path)).expect("open ledger");
    let entries = ledger.entries("n1-smoke");
    let attest = entries
        .iter()
        .find(|entry| entry.kind == EvidenceKind::ToolAttestation)
        .expect("ToolAttestation must be appended");
    assert!(
        attest.recipe_digest.is_some(),
        "recipe_digest must be bound: {attest:?}"
    );
    assert!(
        attest.policy_digest.is_some(),
        "policy_digest must be bound: {attest:?}"
    );
    assert!(
        attest.commit_sha.is_some(),
        "commit_sha must be bound: {attest:?}"
    );

    let _ = std::fs::remove_file(&ledger_path);
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
    std::fs::create_dir_all(dir.join("ws")).expect("ws fixture");
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
    std::fs::create_dir_all(dir.join("ws")).expect("ws fixture");
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

/// Прогон без `.git` в предках cwd обязан отказать названно и НЕ писать
/// `ToolAttestation`: привязка к коммиту — часть удостоверения, а не украшение.
#[test]
fn run_without_git_head_refuses() {
    let dir = scratch_dir("no-git");
    let home = dir.join("home");
    std::fs::create_dir_all(&home).expect("create home");
    let cwd = dir.join("scratch");
    std::fs::create_dir_all(&cwd).expect("create scratch cwd");
    std::fs::create_dir_all(dir.join("ws")).expect("ws fixture");
    let task_path = dir.join("task.toml");
    let toml = "[task]\nid = \"n1-no-git\"\ngoal = \"echo hello\"\nworkspace_ref = \"ws\"\nmodel_ref = \"m\"\npacket = \"V2/01\"\nprofile = \"pi\"\nevidence_policy = \"e\"\n\n[task.limits]\nmax_files = 1\nmax_diff_lines = 1\ntimeout_seconds = 30\nmax_retries = 0\ncontext_budget_tokens = 1\ndata_class = \"internal\"\n";
    std::fs::write(&task_path, toml).expect("write task");
    let pi = fake_pi(&dir, "FAKE_PI_INVOKED");

    let result = run_cli_with_cwd(&task_path, Some(&pi), &home, &cwd);
    assert_ne!(
        result.status,
        Some(0),
        "a run without a git HEAD must refuse; stdout: {}",
        result.stdout
    );
    assert!(
        !result.stdout.contains("verdict="),
        "stdout must not claim a verdict: {}",
        result.stdout
    );
    assert!(
        result.stderr.contains("MissingAttestationBinding"),
        "stderr must name the missing binding: {}",
        result.stderr
    );
    let ledger_path = field(&result.stderr, "ledger")
        .expect("refusal must print ledger= to stderr")
        .to_string();
    let ledger = SqliteEvidenceLedger::open(Path::new(&ledger_path)).expect("open refusal ledger");
    let entries = ledger.entries("n1-no-git");
    assert!(
        !entries
            .iter()
            .any(|entry| entry.kind == EvidenceKind::ToolAttestation),
        "no ToolAttestation may be appended on refusal: {entries:?}"
    );
}

/// Связанная worktree: `.git` — файл-указатель, `HEAD` и `commondir` лежат в
/// каталоге worktree, а ссылка — в общем `.git`. Без `commondir` коммит бы не
/// нашёлся, поэтому синтетическая раскладка фиксирует эту ветку без `git worktree`.
#[test]
fn run_attestation_reads_linked_worktree_commit() {
    const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
    let dir = scratch_dir("linked-worktree");
    let home = dir.join("home");
    std::fs::create_dir_all(&home).expect("create home");
    let main_git = dir.join("main/.git");
    let gitdir = main_git.join("worktrees/w1");
    std::fs::create_dir_all(&gitdir).expect("create worktree gitdir");
    std::fs::write(gitdir.join("HEAD"), "ref: refs/heads/main\n").expect("write HEAD");
    std::fs::write(gitdir.join("commondir"), "../..\n").expect("write commondir");
    std::fs::create_dir_all(main_git.join("refs/heads")).expect("create refs");
    std::fs::write(main_git.join("refs/heads/main"), format!("{COMMIT}\n")).expect("write ref");
    let worktree = dir.join("wt");
    std::fs::create_dir_all(&worktree).expect("create worktree");
    std::fs::write(worktree.join(".git"), "gitdir: ../main/.git/worktrees/w1\n")
        .expect("write .git pointer");
    std::fs::create_dir_all(dir.join("ws")).expect("ws fixture");
    let task_path = dir.join("task.toml");
    let toml = "[task]\nid = \"n1-worktree\"\ngoal = \"echo hello\"\nworkspace_ref = \"ws\"\nmodel_ref = \"m\"\npacket = \"V2/01\"\nprofile = \"pi\"\nevidence_policy = \"e\"\n\n[task.limits]\nmax_files = 1\nmax_diff_lines = 1\ntimeout_seconds = 30\nmax_retries = 0\ncontext_budget_tokens = 1\ndata_class = \"internal\"\n";
    std::fs::write(&task_path, toml).expect("write task");
    let pi = fake_pi(&dir, "FAKE_PI_INVOKED");

    let result = run_cli_with_cwd(&task_path, Some(&pi), &home, &worktree);
    assert_eq!(result.status, Some(0), "stderr: {}", result.stderr);
    let ledger_path = field(&result.stdout, "ledger")
        .expect("run prints its ledger path")
        .to_string();
    let ledger = SqliteEvidenceLedger::open(Path::new(&ledger_path)).expect("open ledger");
    let entries = ledger.entries("n1-worktree");
    let attest = entries
        .iter()
        .find(|entry| entry.kind == EvidenceKind::ToolAttestation)
        .expect("ToolAttestation must be appended");
    assert_eq!(
        attest.commit_sha.as_ref().map(|sha| sha.0.as_str()),
        Some(COMMIT),
        "commit_sha must come from the common dir through commondir: {attest:?}"
    );

    let _ = std::fs::remove_file(&ledger_path);
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

/// Подставной Pi для сквозного сокетного прогона: говорит по unix-сокетам каналов
/// модели и гейта. Скрипт на Python, потому что shell без внешних утилит кадры не
/// шлёт. Он спрашивает гейт о `read`, отказывается продолжать при ответе не `allow`
/// и только потом делает ход модели; события на stdout — ровно доноры `Progress`.
fn socket_pi(dir: &Path) -> PathBuf {
    let pi = dir.join("pi-sockets");
    let script = r#"#!/usr/bin/env python3
import json, os, socket, sys

def connect(path):
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.settimeout(15)
    s.connect(path)
    return s

def gate(path):
    s = connect(path)
    s.sendall(b'{"kind":"tool_call","tool":"read","parent_tool_call_id":null}\n')
    buf = b""
    while b"\n" not in buf:
        chunk = s.recv(65536)
        if not chunk:
            break
        buf += chunk
    s.close()
    return json.loads(buf.split(b"\n", 1)[0].decode()).get("kind")

def model(path, text):
    s = connect(path)
    frame = json.dumps({"kind": "turn", "tools": [], "messages": [{"role": "user", "text": text}]})
    s.sendall((frame + "\n").encode())
    buf = b""
    while True:
        if b"\n" in buf:
            line, buf = buf.split(b"\n", 1)
            if json.loads(line.decode()).get("kind") == "done":
                break
            continue
        chunk = s.recv(65536)
        if not chunk:
            break
        buf += chunk
    s.close()

answer = gate(os.environ["STELLWERK_GATE_SOCK"])
if answer != "allow":
    sys.stderr.write("gate answered %s\n" % answer)
    sys.exit(3)
model(os.environ["STELLWERK_MODEL_SOCK"], "go")
for event in (
    '{"type":"tool_execution_start","toolCallId":"t1","toolName":"read"}',
    '{"type":"message_end","message":{"role":"assistant","usage":{"input":10},"stopReason":"toolUse"}}',
    '{"type":"message_end","message":{"role":"assistant","usage":{"input":10},"stopReason":"stop"}}',
    '{"type":"turn_end","message":{"role":"assistant","usage":{"input":10},"stopReason":"stop"}}',
):
    print(event)
"#;
    std::fs::write(&pi, script).expect("write socket pi");
    let mut mode = std::fs::metadata(&pi).expect("pi metadata").permissions();
    mode.set_mode(0o755);
    std::fs::set_permissions(&pi, mode).expect("pi executable");
    pi
}

/// Локальный HTTP-бэкенд, считающий ходы. Потолок не «сколько успели»: цикл идёт до
/// остановки, поэтому после прогона счётчик — точное число model turn'ов.
struct CountingBackend {
    base_url: String,
    requests: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    worker: std::thread::JoinHandle<()>,
}

fn serve_counting(bodies: &[&str]) -> CountingBackend {
    let listener = TcpListener::bind("127.0.0.1:0").expect("локальный порт");
    let port = listener.local_addr().expect("адрес").port();
    let requests = Arc::new(AtomicUsize::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let (counted, stopper, bodies) = (
        Arc::clone(&requests),
        Arc::clone(&stop),
        bodies
            .iter()
            .map(|body| body.to_string())
            .collect::<Vec<_>>(),
    );
    let worker = std::thread::spawn(move || {
        listener.set_nonblocking(true).expect("неблокирующий");
        while !stopper.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream.set_nonblocking(false).expect("блокирующий");
                    let turn = counted.fetch_add(1, Ordering::SeqCst);
                    let body = &bodies[turn.min(bodies.len() - 1)];
                    let mut chunk = [0u8; 4096];
                    let read = stream.read(&mut chunk).expect("запрос прочитан");
                    assert!(read > 0, "пустой запрос к модели");
                    let response = format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    stream
                        .write_all(response.as_bytes())
                        .expect("ответ записан");
                    stream.flush().expect("ответ отправлен");
                }
                Err(why) if why.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(why) => panic!("соединение модели: {why}"),
            }
        }
    });
    CountingBackend {
        base_url: format!("http://127.0.0.1:{port}/v1"),
        requests,
        stop,
        worker,
    }
}

/// Сквозной сокетный прогон: fake Pi спрашивает гейт о `read`, получает `allow`,
/// делает ровно один ход модели. Счётчики `RunResult` — не косвенность: модель
/// считает бэкенд, гейт — обслуженные заявки в `run_task_with`.
#[test]
fn run_task_with_drives_one_model_turn_and_one_gate_request() {
    let dir = scratch_dir("channels");
    let token = dir.join("neuraldeep.token");
    std::fs::write(&token, "test-token\n").expect("токен");
    std::fs::create_dir_all(dir.join("ws")).expect("ws fixture");
    let pi = socket_pi(&dir);
    let backend = serve_counting(&[r#"{"choices":[{"message":{"content":"pong"}}]}"#]);
    let task_path = dir.join("task.toml");
    let toml = "[task]\nid = \"v2-05a\"\ngoal = \"echo hello\"\nworkspace_ref = \"ws\"\nmodel_ref = \"m\"\npacket = \"V2/05a\"\nprofile = \"pi\"\nevidence_policy = \"e\"\n\n[task.limits]\nmax_files = 1\nmax_diff_lines = 1\ntimeout_seconds = 30\nmax_retries = 0\ncontext_budget_tokens = 1\ndata_class = \"internal\"\n";
    std::fs::write(&task_path, toml).expect("write fixture");

    let config = ModelConfig::with_token_path(&backend.base_url, HOST_MODEL_ID, &token)
        .map_err(|e| e.to_string())
        .expect("model config");
    let result = run_task_with(&task_path, config, &pi, &pi_extension()).expect("run");

    backend.stop.store(true, Ordering::SeqCst);
    backend.worker.join().expect("поток бэкенда");

    assert_eq!(
        backend.requests.load(Ordering::SeqCst),
        1,
        "бэкенд модели получил ровно один ход"
    );
    assert_eq!(result.model_turns, 1, "один ход модели обслужен");
    assert_eq!(result.gate_requests, 1, "ровно одна заявка в гейт");
    assert_eq!(result.verdict, "Allow", "прогон разрешён");

    let _ = std::fs::remove_file(&result.store_path);
    let _ = std::fs::remove_file(&result.ledger_path);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Отсутствие настройки модели — названный отказ ДО прогона, а не фикстура и не
/// пустой вердикт. `HOME`/`XDG_CONFIG_HOME` сняты, значит путь токена не выводится.
#[test]
fn run_task_without_model_config_refuses_by_name() {
    let output = Command::new(env!("CARGO_BIN_EXE_stellwerk"))
        .arg("run")
        .arg("does-not-matter.toml")
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("HOME")
        .env_remove("PI_BIN")
        .env_remove("STELLWERK_PI_EXTENSION")
        .output()
        .expect("cli binary should spawn");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_ne!(output.status.code(), Some(0), "stderr: {stderr}");
    assert!(
        !String::from_utf8_lossy(&output.stdout).contains("verdict="),
        "stdout must not claim a verdict"
    );
    assert!(
        stderr.contains("model setting") && stderr.contains("is not configured"),
        "stderr must name the unconfigured model setting: {stderr}"
    );
}

/// Подставной Pi, доказывающий копию workspace: сравнивает `readme.txt` из
/// текущего каталога (это `work`) с маркером и падает, если в `work` попал
/// `.git`. Пустой `work` не даст валидного прогона.
fn workspace_pi(dir: &Path, marker: &str) -> PathBuf {
    let pi = dir.join("pi-workspace");
    let script = format!(
        r#"#!/bin/sh
[ "$(cat readme.txt)" = '{marker}' ] || exit 7
[ ! -e .git ] || exit 8
printf '{{"type":"tool_execution_start","toolCallId":"w1","toolName":"read"}}\n'
printf '{{"type":"message_end","message":{{"role":"assistant","usage":{{"input":10}},"stopReason":"toolUse"}}}}\n'
printf '{{"type":"message_end","message":{{"role":"assistant","usage":{{"input":10}},"stopReason":"stop"}}}}\n'
printf '{{"type":"turn_end","message":{{"role":"assistant","usage":{{"input":10}},"stopReason":"stop"}}}}\n'
"#
    );
    std::fs::write(&pi, script).expect("write workspace pi");
    let mut mode = std::fs::metadata(&pi).expect("pi metadata").permissions();
    mode.set_mode(0o755);
    std::fs::set_permissions(&pi, mode).expect("pi executable");
    pi
}

/// Абсолютный `workspace_ref` копируется в `work` до запуска агента: `readme.txt`
/// виден, `.git` — нет. Пустой `work` стал бы успехом без копии, и этот тест его
/// ловит: без файла `cat` в фикстуре падает.
#[test]
fn run_against_workspace_copy_sees_files() {
    let dir = scratch_dir("workspace-copy");
    let ws = dir.join("ws");
    std::fs::create_dir_all(ws.join(".git")).expect("ws git");
    std::fs::write(ws.join(".git/HEAD"), "ref: refs/heads/main\n").expect("git head");
    let marker = format!("workspace-copy-{}", dir.display());
    std::fs::write(ws.join("readme.txt"), &marker).expect("readme");

    let task_path = dir.join("task.toml");
    let toml = format!("[task]\nid = \"v2-05b-copy\"\ngoal = \"echo hello\"\nworkspace_ref = \"{}\"\nmodel_ref = \"m\"\npacket = \"V2/05b\"\nprofile = \"pi\"\nevidence_policy = \"e\"\n\n[task.limits]\nmax_files = 1\nmax_diff_lines = 1\ntimeout_seconds = 30\nmax_retries = 0\ncontext_budget_tokens = 1\ndata_class = \"internal\"\n", ws.display());
    std::fs::write(&task_path, toml).expect("write task");

    let token = dir.join("neuraldeep.token");
    std::fs::write(&token, "test-token\n").expect("token");
    let config = ModelConfig::with_token_path("http://127.0.0.1:9/v1", HOST_MODEL_ID, &token)
        .expect("model config");
    let pi = workspace_pi(&dir, &marker);

    let result = run_task_with(&task_path, config, &pi, &pi_extension())
        .expect("run over a workspace copy must be a run, not an error");
    assert_eq!(result.verdict, "Allow", "copy run must be valid");

    let _ = std::fs::remove_file(&result.store_path);
    let _ = std::fs::remove_file(&result.ledger_path);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Отсутствующий `workspace_ref` — названный `MissingWorkspace`, а не пустой
/// прогон: агент не должен стартовать без дерева, о котором задача говорит.
#[test]
fn missing_workspace_is_named_refusal() {
    let dir = scratch_dir("missing-workspace");
    let task_path = dir.join("task.toml");
    let absent = dir.join("ws-absent");
    let toml = format!("[task]\nid = \"v2-05b-missing\"\ngoal = \"echo hello\"\nworkspace_ref = \"{}\"\nmodel_ref = \"m\"\npacket = \"V2/05b\"\nprofile = \"pi\"\nevidence_policy = \"e\"\n\n[task.limits]\nmax_files = 1\nmax_diff_lines = 1\ntimeout_seconds = 30\nmax_retries = 0\ncontext_budget_tokens = 1\ndata_class = \"internal\"\n", absent.display());
    std::fs::write(&task_path, toml).expect("write task");

    let token = dir.join("neuraldeep.token");
    std::fs::write(&token, "test-token\n").expect("token");
    let config = ModelConfig::with_token_path("http://127.0.0.1:9/v1", HOST_MODEL_ID, &token)
        .expect("model config");
    let pi = fake_pi(&dir, "FAKE_PI_INVOKED");

    let error = match run_task_with(&task_path, config, &pi, &pi_extension()) {
        Ok(_) => panic!("a missing workspace must refuse, not run"),
        Err(error) => error,
    };
    assert!(
        error.contains("MissingWorkspace"),
        "the refusal must be named: {error}"
    );
    assert!(
        error.contains(&absent.display().to_string()),
        "the refusal must name the resolved path: {error}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// Настоящий Pi: `PI_BIN`, иначе `pi` из PATH. Отсутствие — провал с точным
/// путём, а не пропуск.
fn real_pi() -> PathBuf {
    if let Some(value) = std::env::var_os("PI_BIN") {
        let path = PathBuf::from(value);
        assert!(path.is_file(), "real Pi missing: {}", path.display());
        return path;
    }
    let path = std::env::var_os("PATH").expect("PATH is set");
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join("pi");
        if candidate.is_file() {
            return candidate;
        }
    }
    panic!("real Pi missing: PI_BIN is unset and no `pi` is on PATH");
}

/// Копия managed-layout Pi: launcher и ссылка на `install`. Sandbox монтирует
/// только каталоги переданной программы, поэтому настоящий Pi должен лежать под
/// путём, который `run_task_with` получает как `pi`, а не по хостовому пути.
fn pi_layout(dir: &Path, real: &Path) -> PathBuf {
    let managed = std::fs::canonicalize(real).expect("canonical real Pi");
    let agent = managed
        .parent()
        .and_then(Path::parent)
        .expect("managed agent dir");
    let layout = dir.join("agent");
    std::fs::create_dir_all(layout.join("bin")).expect("layout bin");
    let launcher = layout.join("bin/pi");
    std::fs::copy(&managed, &launcher).expect("copy launcher");
    let mut mode = std::fs::metadata(&launcher)
        .expect("launcher metadata")
        .permissions();
    mode.set_mode(0o755);
    std::fs::set_permissions(&launcher, mode).expect("launcher executable");
    std::os::unix::fs::symlink(agent.join("install"), layout.join("install"))
        .expect("install link");
    launcher
}

/// `run_task_with` подаёт transport-argv, но не провайдера и не первый ход.
/// Обёртка задаёт их и исполняет настоящий launcher — версия проверяется у того
/// же бинаря, а не у подставного скрипта.
fn real_pi_launcher(launcher: &Path) -> PathBuf {
    let wrapper = launcher.with_file_name("pi-real");
    std::fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nexec {} \"$@\" --provider stellwerk --model proxy --mode json -p go\n",
            launcher.display()
        ),
    )
    .expect("write real Pi wrapper");
    let mut mode = std::fs::metadata(&wrapper)
        .expect("wrapper metadata")
        .permissions();
    mode.set_mode(0o755);
    std::fs::set_permissions(&wrapper, mode).expect("wrapper executable");
    wrapper
}

/// Настоящий Pi 1.0.2 через `run_task_with`: локальный backend отвечает `read`,
/// Pi читает `readme.txt` из копии workspace и останавливается. Отсутствие Pi —
/// провал, не пропуск.
#[test]
fn run_task_with_real_pi() {
    let dir = scratch_dir("real-pi");
    let real = real_pi();
    let version = Command::new(&real)
        .arg("--version")
        .output()
        .expect("pi --version");
    let version = String::from_utf8_lossy(&version.stdout).trim().to_owned();
    assert_eq!(
        version,
        "1.0.2",
        "real Pi must be 1.0.2 at {}: {version:?}",
        real.display()
    );
    eprintln!("real Pi {} -> {version}", real.display());

    let ws = dir.join("ws");
    std::fs::create_dir_all(ws.join(".git")).expect("ws git");
    std::fs::write(ws.join(".git/HEAD"), "ref: refs/heads/main\n").expect("git head");
    let marker = format!("real-pi-{}", dir.display());
    std::fs::write(ws.join("readme.txt"), &marker).expect("readme");

    let task_path = dir.join("task.toml");
    let toml = format!("[task]\nid = \"v2-05b-real\"\ngoal = \"echo hello\"\nworkspace_ref = \"{}\"\nmodel_ref = \"m\"\npacket = \"V2/05b\"\nprofile = \"pi\"\nevidence_policy = \"e\"\n\n[task.limits]\nmax_files = 1\nmax_diff_lines = 1\ntimeout_seconds = 30\nmax_retries = 0\ncontext_budget_tokens = 1\ndata_class = \"internal\"\n", ws.display());
    std::fs::write(&task_path, toml).expect("write task");

    let token = dir.join("neuraldeep.token");
    std::fs::write(&token, "test-token\n").expect("token");
    let backend = serve_counting(&[
        r#"{"choices":[{"message":{"content":null,"tool_calls":[{"id":"r1","function":{"name":"read","arguments":"{\"path\":\"readme.txt\"}"}}]}}]}"#,
        r#"{"choices":[{"message":{"content":"done"}}]}"#,
    ]);
    let config = ModelConfig::with_token_path(&backend.base_url, HOST_MODEL_ID, &token)
        .expect("model config");
    let pi = real_pi_launcher(&pi_layout(&dir, &real));

    let result =
        run_task_with(&task_path, config, &pi, &pi_extension()).expect("real Pi run must complete");

    backend.stop.store(true, Ordering::SeqCst);
    backend.worker.join().expect("backend thread");

    assert_eq!(result.verdict, "Allow", "real Pi run must be valid");
    assert_eq!(result.model_turns, 2, "two turns: read and stop");
    assert!(result.gate_requests >= 1, "read passed the gate");

    let _ = std::fs::remove_file(&result.store_path);
    let _ = std::fs::remove_file(&result.ledger_path);
    let _ = std::fs::remove_dir_all(&dir);
}
