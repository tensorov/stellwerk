//! N2-00h3b: площадка Build — копия без истории, политика C и слуга каналов.
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use std::collections::VecDeque;
use std::sync::Mutex;

use serde_json::json;
use stellwerk_cli::build_runner::{
    copy_workspace, run_build, serve_gate_until_stopped, serve_model_until_stopped, wake,
    BuildPolicy, BuildRun, BUILD_MODULE,
};
use stellwerk_cli::plan::BuildRequest;
use stellwerk_gateway::gate::decide;
use stellwerk_task::m5::gate_channel::{GateAnswer, GateChannel, GatePolicy, PolicyVerdict};
use stellwerk_task::m5::model_transport::{
    BackendError, Frame, ModelBackend, ModelProxy, TurnMessage,
};
use stellwerk_task::m5::session_identity::{CodemodeState, SessionDescription, ToolSet};

static SEQ: AtomicUsize = AtomicUsize::new(0);

fn scratch(name: &str) -> PathBuf {
    let seq = SEQ.fetch_add(1, Ordering::SeqCst);
    let root = std::env::temp_dir().join(format!("sw-h3b-{name}-{}-{seq}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("каталог пробега");
    root
}

fn binding() -> stellwerk_core::contract::ModelBinding {
    serde_json::from_str(
        r#"{"id":"b1","provider":"neuraldeep","model_id":"qwen3.6-unlim-xl-noreason","data_class":"public","retention":"first_party","context_window":138336,"max_output":41500,"budget_tokens_per_task":100000}"#,
    )
    .expect("binding")
}

fn session(held: &[&str]) -> SessionDescription {
    SessionDescription::new(
        "build-1",
        Path::new("/tmp"),
        ToolSet::listed(held).expect("набор инструментов"),
        Duration::from_secs(60),
        CodemodeState::AbsentAfterConfigLoad,
    )
    .expect("описание сессии")
}

fn verdict_for(tool: &str) -> String {
    let policy = BuildPolicy { model: binding() };
    let adjudication = session(&["read", "bash", "edit", "patch"])
        .adjudicate(tool, None)
        .expect("инструмент в наборе");
    let request = policy
        .request_for(&adjudication)
        .expect("политика отвечает всегда");
    format!("{:?}", decide(&request))
}

/// История в песочницу не попадает: её там не было, а не «её удалили».
#[test]
fn a_workspace_copy_carries_no_history() {
    let root = scratch("copy");
    let repo = root.join("repo");
    std::fs::create_dir_all(repo.join(".git")).expect(".git");
    std::fs::write(repo.join(".git/HEAD"), "ref: refs/heads/main\n").expect("HEAD");
    std::fs::create_dir_all(repo.join("src")).expect("src");
    std::fs::write(repo.join("src/lib.rs"), "fn main() {}\n").expect("lib.rs");
    std::fs::write(repo.join("Cargo.toml"), "[package]\n").expect("манифест");

    let dest = root.join("work");
    copy_workspace(&repo, &dest).expect("копия");

    assert!(!dest.join(".git").exists(), "история не скопирована");
    assert_eq!(
        std::fs::read_to_string(dest.join("src/lib.rs")).expect("файл"),
        "fn main() {}\n"
    );
    assert!(dest.join("Cargo.toml").is_file(), "манифест на месте");
}

/// C: чтение разрешено, оболочка запрещена безусловно.
#[test]
fn read_is_allowed_and_the_shell_is_denied() {
    assert_eq!(verdict_for("read"), "Allow");
    let shell = verdict_for("bash");
    assert!(shell.starts_with("Deny"), "оболочка запрещена: {shell}");
    assert!(shell.contains("R5"), "причина называет класс: {shell}");
}

/// Инструмент вне доверенного реестра — отказ с причиной, а не молчаливый `Deny`.
#[test]
fn an_unknown_tool_is_denied_with_a_reason() {
    let unknown = verdict_for("patch");
    assert!(unknown.starts_with("Deny"), "{unknown}");
    assert!(
        unknown.contains("unclassified_tool"),
        "причина названа: {unknown}"
    );
}

/// Слуга гейта выходит по флагу и пустому подключению, а не висит в `accept`.
#[test]
fn the_gate_servant_stops_on_a_wake_connection() {
    let root = scratch("gate");
    let channel = GateChannel::bind(&root).expect("канал гейта");
    let socket = channel.socket().to_path_buf();
    let stop = AtomicBool::new(false);
    let policy = BuildPolicy { model: binding() };
    let held = session(&["read"]);

    std::thread::scope(|scope| {
        let servant = scope.spawn(|| {
            serve_gate_until_stopped(&channel, &held, &policy, &stop).expect("служба гейта")
        });
        stop.store(true, Ordering::SeqCst);
        wake(&socket).expect("пробуждение");
        assert_eq!(servant.join().expect("поток"), 0, "заявок не было");
    });
}

struct Silent;

impl ModelBackend for Silent {
    fn turn(&self, _messages: &[TurnMessage]) -> Result<Vec<Frame>, BackendError> {
        Err(BackendError {
            message: "canary не ходит в модель".to_string(),
        })
    }
}

/// Тот же контракт остановки для канала модели.
#[test]
fn the_model_servant_stops_on_a_wake_connection() {
    let root = scratch("model");
    let proxy = ModelProxy::bind(&root).expect("канал модели");
    let socket = proxy.socket().to_path_buf();
    let stop = AtomicBool::new(false);

    std::thread::scope(|scope| {
        let servant = scope
            .spawn(|| serve_model_until_stopped(&proxy, &Silent, &stop).expect("служба модели"));
        stop.store(true, Ordering::SeqCst);
        wake(&socket).expect("пробуждение");
        assert_eq!(servant.join().expect("поток"), 0, "ходов не было");
    });
}

/// Модуль, от имени которого гейт видит инструменты, — не имя процесса агента.
#[test]
fn the_gate_speaks_for_the_build_module() {
    assert_eq!(BUILD_MODULE, "stellwerk-build");
}

/// Ответы модели по очереди. Пустая очередь — не отказ прогона: агент мог закончить раньше.
struct Scripted(Mutex<VecDeque<Vec<Frame>>>);

impl ModelBackend for Scripted {
    fn turn(&self, _messages: &[TurnMessage]) -> Result<Vec<Frame>, BackendError> {
        let mut queue = self.0.lock().expect("очередь");
        Ok(queue.pop_front().unwrap_or_else(|| {
            vec![Frame::Text {
                text: "готово".to_string(),
            }]
        }))
    }
}

fn request() -> BuildRequest {
    BuildRequest {
        goal: "добавить строку".to_string(),
        allowed_changes: vec!["src/lib.rs".to_string()],
        acceptance: vec!["тест проходит".to_string()],
        checks: vec!["cargo test".to_string()],
    }
}

fn pi() -> PathBuf {
    let home = std::env::var_os("HOME").expect("HOME задан");
    let launcher = std::env::var_os("PI_BIN").map_or_else(
        || PathBuf::from(home).join(".pi/agent/bin/pi"),
        PathBuf::from,
    );
    assert!(
        launcher.is_file(),
        "нет {}; задайте PI_BIN",
        launcher.display()
    );
    launcher
}

fn extension() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../stellwerk-task/pi/stellwerk_transport.ts")
}

fn repo(name: &str) -> (PathBuf, PathBuf) {
    let root = scratch(name);
    let repo = root.join("repo");
    std::fs::create_dir_all(repo.join(".git")).expect(".git");
    std::fs::write(repo.join(".git/HEAD"), "ref: refs/heads/main\n").expect("HEAD");
    std::fs::create_dir_all(repo.join("src")).expect("src");
    std::fs::write(repo.join("src/lib.rs"), "pub fn one() -> u8 { 1 }\n").expect("lib.rs");
    (repo, root.join("run"))
}

/// C: `pi` действительно стартует в песочнице и говорит через наш канал.
#[test]
fn the_build_runs_pi_through_our_channels() {
    let (repo, root) = repo("run");
    let backend = Scripted(Mutex::new(VecDeque::from([vec![Frame::Text {
        text: "pong".to_string(),
    }]])));
    let outcome = run_build(
        BuildRun {
            request: &request(),
            repo: &repo,
            root: &root,
            extension: &extension(),
            pi: &pi(),
        },
        &backend,
    )
    .expect("прогон");

    assert!(
        outcome.output.contains("pong"),
        "вывод pi: {}",
        outcome.output
    );
    assert!(
        outcome.model_turns >= 1,
        "ходов модели: {}",
        outcome.model_turns
    );
    assert!(
        !root.join("work/.git").exists(),
        "история не попала в песочницу"
    );
}

/// C целиком: оболочка запрещена до эффекта — канарейка не создана, причина названа.
#[test]
fn the_shell_is_denied_before_it_touches_anything() {
    let (repo, root) = repo("deny");
    let canary = root.join("work/canary");
    let call = Frame::ToolCall {
        id: "call-1".to_string(),
        name: "bash".to_string(),
        input: json!({ "command": format!("touch {}", canary.display()) }),
    };
    let backend = Scripted(Mutex::new(VecDeque::from([
        vec![call],
        vec![Frame::Text {
            text: "ok".to_string(),
        }],
    ])));
    let outcome = run_build(
        BuildRun {
            request: &request(),
            repo: &repo,
            root: &root,
            extension: &extension(),
            pi: &pi(),
        },
        &backend,
    )
    .expect("прогон");

    assert!(!canary.exists(), "эффекта нет: {}", outcome.output);
    assert!(
        outcome.output.contains("gate_denied"),
        "причина deny: {}",
        outcome.output
    );
    assert!(
        outcome.gate_requests >= 1,
        "заявок в гейт: {}",
        outcome.gate_requests
    );
}

fn adjudicate(tool: &str) -> stellwerk_task::m5::session_identity::Adjudication {
    session(&["read", "bash", "edit", "write"])
        .adjudicate(tool, None)
        .expect("инструмент в наборе")
}

/// Запись разрешена уровнем R1, и пришедший путь её не отменяет: путь — след вызова,
/// а не второй фильтр. Умолчание трейта здесь отказало бы, поэтому проверяем явно.
#[test]
fn an_edit_is_allowed_by_level_and_the_path_does_not_revoke_it() {
    let policy = BuildPolicy { model: binding() };
    let call = adjudicate("edit");
    let verdict = policy.decide_call(&call, Some("src/lib.rs"));
    let PolicyVerdict::Evaluated { request, .. } = verdict else {
        panic!("ожидалось решение: {verdict:?}");
    };
    assert!(
        matches!(decide(&request), stellwerk_gateway::gate::Verdict::Allow),
        "R1 разрешён таблицей"
    );

    let call = adjudicate("write");
    let verdict = policy.decide_call(&call, None);
    let PolicyVerdict::Evaluated { request, .. } = verdict else {
        panic!("ожидалось решение: {verdict:?}");
    };
    assert!(matches!(
        decide(&request),
        stellwerk_gateway::gate::Verdict::Allow
    ));
}

/// Оболочка остаётся R5: уровень не совпадает с тем, что разрешает план, и запрет
/// безусловен.
#[test]
fn the_shell_stays_denied_at_r5() {
    let policy = BuildPolicy { model: binding() };
    let verdict = policy.decide_call(&adjudicate("bash"), Some("src/lib.rs"));
    let PolicyVerdict::Evaluated { request, .. } = verdict else {
        panic!("ожидалось решение: {verdict:?}");
    };
    let denied = decide(&request);
    assert!(
        matches!(denied, stellwerk_gateway::gate::Verdict::Deny { .. }),
        "bash: {denied:?}"
    );
}

/// Живой прогон: разрешение остаётся в журнале, а не только в счётчике заявок.
#[test]
fn the_build_logs_an_allowed_read() {
    let (repo, root) = repo("edit");
    let call = Frame::ToolCall {
        id: "call-1".to_string(),
        name: "read".to_string(),
        input: json!({ "path": "src/lib.rs" }),
    };
    let backend = Scripted(Mutex::new(VecDeque::from([
        vec![call],
        vec![Frame::Text {
            text: "ok".to_string(),
        }],
    ])));
    let outcome = run_build(
        BuildRun {
            request: &request(),
            repo: &repo,
            root: &root,
            extension: &extension(),
            pi: &pi(),
        },
        &backend,
    )
    .expect("прогон");

    assert!(
        outcome
            .gate
            .iter()
            .any(|answer| matches!(answer, GateAnswer::Allow { tool } if tool == "read")),
        "журнал гейта: {:?}",
        outcome.gate
    );
}

/// C целиком: правка разрешена уровнем R1 и доезжает до одноразовой копии. Схема
/// входа — настоящая: Pi проверяет аргументы до вызова хука, и кадр без `edits`
/// до гейта не доходит.
#[test]
fn an_edit_reaches_the_copy_and_the_gate_authorizes_it() {
    let (repo, root) = repo("write");
    let call = Frame::ToolCall {
        id: "call-1".to_string(),
        name: "edit".to_string(),
        input: json!({
            "path": "src/lib.rs",
            "edits": [{
                "oldText": "pub fn one() -> u8 { 1 }",
                "newText": "pub fn one() -> u8 { 2 }"
            }]
        }),
    };
    let backend = Scripted(Mutex::new(VecDeque::from([
        vec![call],
        vec![Frame::Text {
            text: "ok".to_string(),
        }],
    ])));
    let outcome = run_build(
        BuildRun {
            request: &request(),
            repo: &repo,
            root: &root,
            extension: &extension(),
            pi: &pi(),
        },
        &backend,
    )
    .expect("прогон");

    assert!(
        outcome
            .gate
            .iter()
            .any(|answer| matches!(answer, GateAnswer::Allow { tool } if tool == "edit")),
        "журнал гейта: {:?}",
        outcome.gate
    );
    assert!(
        !outcome.output.contains("gate_denied"),
        "гейт не отказывал: {}",
        outcome.output
    );
    let edited = std::fs::read_to_string(root.join("work/src/lib.rs")).expect("копия на месте");
    assert!(edited.contains("{ 2 }"), "правка в копии: {edited}");
    let original = std::fs::read_to_string(repo.join("src/lib.rs")).expect("репозиторий");
    assert!(
        original.contains("{ 1 }"),
        "репозиторий не тронут: {original}"
    );
}
