//! N2-00h3b: площадка Build — копия без истории, политика C и слуга каналов.
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use std::collections::VecDeque;
use std::sync::Mutex;

use serde_json::json;
use stellwerk_cli::build_runner::{
    answer_text, build_args, copy_workspace, gate_summary, prompt, run_build,
    serve_gate_until_stopped, serve_model_until_stopped, wake, BuildPolicy, BuildRun, BUILD_HELD,
    BUILD_MODULE, BUILD_OFFERED,
};
use stellwerk_cli::plan::BuildRequest;
use stellwerk_gateway::gate::{decide, ContentDigest};
use stellwerk_task::m5::gate_channel::{GateAnswer, GateChannel, GatePolicy, PolicyVerdict};
use stellwerk_task::m5::model_transport::{
    BackendError, Frame, ModelBackend, ModelProxy, TurnMessage, TurnRequest,
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

    // Build предлагает read/edit/write: сценарий принимает ход вместе с набором.
    fn turn_request(&self, request: &TurnRequest) -> Result<Vec<Frame>, BackendError> {
        self.turn(&request.messages)
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

/// Модели предлагается ровно то, что она вправе вызвать: оболочки в списке нет.
#[test]
fn the_build_does_not_offer_bash() {
    assert_eq!(BUILD_OFFERED.to_vec(), vec!["read", "edit", "write"]);

    let args = build_args(Path::new("/tmp/stellwerk_transport.ts"));
    assert!(
        args.iter().any(|arg| arg == "--no-builtin-tools"),
        "read/edit/write — встроенные, явный список не нужен: {args:?}"
    );
    assert!(
        !args.iter().any(|arg| arg == "--tools"),
        "фильтрованный список не передаётся: {args:?}"
    );
    let at = args
        .iter()
        .position(|arg| arg == "-e")
        .expect("расширение названо явно");
    assert_eq!(
        args.get(at + 1).map(String::as_str),
        Some("/tmp/stellwerk_transport.ts"),
        "аргументы: {args:?}"
    );
    assert!(
        !args.iter().any(|arg| arg == "bash"),
        "оболочка не предложена: {args:?}"
    );
}

/// Оболочка не предлагается, но гейт её помнит: R5 — это отказ по уровню, отличимый
/// от «инструмента нет в наборе». Иначе неизвестное стало бы разрешением.
#[test]
fn bash_stays_a_gate_level_deny() {
    assert!(
        BUILD_HELD.contains(&"bash"),
        "гейт держит bash: {BUILD_HELD:?}"
    );

    let policy = BuildPolicy { model: binding() };
    let call = session(BUILD_HELD)
        .adjudicate("bash", None)
        .expect("bash есть в наборе гейта");
    let PolicyVerdict::Evaluated { request, .. } = policy.decide_call(&call, None) else {
        panic!("ожидалось решение для bash");
    };
    let denied = decide(&request);
    assert!(
        matches!(denied, stellwerk_gateway::gate::Verdict::Deny { .. }),
        "bash: {denied:?}"
    );

    assert!(
        session(BUILD_HELD).adjudicate("nope", None).is_err(),
        "инструмент вне набора — отсутствие, а не R5-отказ"
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

    let scope = outcome
        .gate
        .iter()
        .find_map(|answer| match answer {
            GateAnswer::Authorized { tool, scope } if tool == "edit" => Some(scope.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("ожидалось Authorized для edit: {:?}", outcome.gate));
    let digest = scope
        .strip_prefix("src/lib.rs#sha256:")
        .unwrap_or_else(|| panic!("scope называет путь и digest: {scope}"));
    assert!(
        ContentDigest::parse(digest).is_ok(),
        "digest в scope — 64 hex: {scope}"
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

/// C: кандидат собирается из байтов workspace, а не из истории репозитория. После
/// правки в нём ровно изменённый файл с digest фактических байтов; неизменённый
/// манифест, копия расширения и `.git` в кандидат не попадают.
#[test]
fn candidate_contains_only_changed_workspace_files() {
    let (repo, root) = repo("candidate");
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

    let expected = ContentDigest::of(b"pub fn one() -> u8 { 2 }\n");
    assert_eq!(
        outcome.candidate.len(),
        1,
        "кандидат: {:?}",
        outcome.candidate
    );
    let entry = &outcome.candidate[0];
    assert_eq!(
        entry.path, "src/lib.rs",
        "кандидат: {:?}",
        outcome.candidate
    );
    assert_eq!(entry.digest, expected.as_hex(), "digest фактических байтов");

    let paths: Vec<&str> = outcome
        .candidate
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    assert!(
        !paths.iter().any(|path| path.contains(".git")),
        "история не в кандидате: {paths:?}"
    );
    assert!(
        !paths.contains(&"stellwerk_transport.ts"),
        "копия расширения — состояние харнесса, не правка: {paths:?}"
    );
    assert!(
        paths.iter().all(|path| {
            let candidate = Path::new(path);
            !candidate.is_absolute()
                && !candidate
                    .components()
                    .any(|part| part == std::path::Component::ParentDir)
        }),
        "пути только относительные и без `..`: {paths:?}"
    );
}

/// Строка статуса называет решения, а не только их число: «никто не спрашивал» и
/// «спрашивали и разрешили» должны читаться по-разному.
#[test]
fn the_gate_summary_names_each_decision() {
    assert_eq!(gate_summary(&[]), "решений нет");
    let summary = gate_summary(&[
        GateAnswer::Allow {
            tool: "read".to_string(),
        },
        GateAnswer::Deny {
            tool: "bash".to_string(),
            reason: "risk_denied: R5".to_string(),
        },
    ]);
    assert_eq!(summary, "allowed(read) · denied(bash:risk_denied: R5)");
}

/// Приглашение требует работы, а не пересказа: без этого модель заканчивает ход
/// текстом, и гейт не видит ни одной заявки.
#[test]
fn the_build_prompt_asks_for_the_work_not_a_description() {
    let text = prompt(&request());
    assert!(text.contains("выполняй"), "приглашение: {text}");
    assert!(text.contains("edit"), "назван инструмент: {text}");
    assert!(
        text.contains("src/lib.rs"),
        "названы допустимые изменения: {text}"
    );
}

/// Вывод Build — поток JSON, и его начало это заголовок сессии, а не ответ модели.
/// Печатать префикс значило показывать не то, что сказал агент.
#[test]
fn the_build_answer_is_the_last_assistant_message() {
    let stream = concat!(
        "{\"type\":\"session\",\"id\":\"01a1\",\"cwd\":\"/tmp/x\"}\n",
        "{\"type\":\"message\",\"message\":{\"role\":\"user\",\"content\":\"задача\"}}\n",
        "{\"type\":\"message\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"правлю\"}]}}\n",
        "{\"type\":\"message\",\"message\":{\"role\":\"assistant\",\"text\":\"готово\"}}\n"
    );
    assert_eq!(answer_text(stream), "готово");
    assert_eq!(answer_text("не json"), "не json");
}
