//! V1/v02: запрещённый вызов обязан оставить названный отказ, а не пустоту.
//!
//! Отказ, записанный как «ничего не произошло», неотличим от прогона, в котором
//! решения не было вовсе. Поэтому проверяются три факта сразу: вердикт `Deny`,
//! отсутствие запуска (а значит и кандидата), и запись в потоке, где названы
//! `status`, `reason` и `actor=policy`.

use std::cell::Cell;
use std::collections::VecDeque;
use std::future::{ready, Future};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Mutex;

use serde_json::json;
use stellwerk_cli::build_runner::{run_build, BuildRun};
use stellwerk_cli::plan::BuildRequest;
use stellwerk_core::contract::{
    DataClass, EventRisk, ModelBinding, PolicyLevel, RetentionClass, RiskClass,
};
use stellwerk_gateway::permission::{PermissionError, ToolRisk};
use stellwerk_store::{EventStore, SqliteStore};
use stellwerk_task::m5::gate_channel::GateAnswer;
use stellwerk_task::m5::model_transport::{
    BackendError, Frame, ModelBackend, TurnMessage, TurnRequest,
};
use stellwerk_task::runner::{
    run_once, Call, CallAttempt, CallVerdict, ExecOutcome, RiskRegistry, Worker, WorkerError,
};

const R5: ToolRisk = ToolRisk::Classified(RiskClass::R5);

/// Счётчик запусков отделён от факта записи: «воркер не вызван» иначе прошло бы по
/// отсутствию события, а отсутствие события неотличимо от забытой записи.
struct FakeWorker {
    launches: Cell<usize>,
    registry: TestRegistry,
}

impl FakeWorker {
    fn new() -> Self {
        Self {
            launches: Cell::new(0),
            registry: TestRegistry,
        }
    }
}

/// Доверенный реестр: `r5` классифицирован здесь, а не самим вызовом. Агент не вправе
/// назвать запрещённый инструмент безопасным.
struct TestRegistry;

impl RiskRegistry for TestRegistry {
    fn risk_of(&self, tool: &str, module: &str) -> Result<ToolRisk, PermissionError> {
        match (tool, module) {
            ("r5", "fs") => Ok(R5),
            _ => Err(PermissionError::TableIncomplete {
                risk: RiskClass::R0,
            }),
        }
    }
}

type Exec<'a> = Pin<Box<dyn Future<Output = Result<ExecOutcome, WorkerError>> + Send + 'a>>;

impl Worker for FakeWorker {
    fn exec(&self, _workspace_ref: &str, _call: &Call) -> Exec<'_> {
        self.launches.set(self.launches.get() + 1);
        Box::pin(ready(Ok(ExecOutcome {
            output: String::new(),
        })))
    }

    fn spawned(&self) -> usize {
        self.launches.get()
    }

    fn registry(&self) -> &dyn RiskRegistry {
        &self.registry
    }
}

/// Binding, пропускающий любой класс данных: иначе отказ по данным замаскировал бы
/// отказ по риску.
fn binding() -> ModelBinding {
    ModelBinding {
        id: "b".into(),
        provider: "neuraldeep".into(),
        model_id: "m".into(),
        data_class: DataClass::Public,
        retention: RetentionClass::FirstParty,
        context_window: 200_000,
        max_output: 8_000,
        budget_tokens_per_task: 43_000,
    }
}

fn refused_attempt() -> CallAttempt {
    CallAttempt {
        task_id: "t-v02".into(),
        call: Call {
            tool: "r5".into(),
            module: "fs".into(),
            risk: R5,
            data: DataClass::Public,
            model: binding(),
            interactive: true,
        },
        caller: PolicyLevel::L5Owner,
        workspace_ref: "ws-stellwerk".into(),
    }
}

fn store() -> SqliteStore {
    SqliteStore::open(Path::new(":memory:")).expect("открыто")
}

fn text_of(event: &stellwerk_core::contract::Event) -> String {
    event
        .payload
        .values()
        .cloned()
        .collect::<Vec<_>>()
        .join(" | ")
}

/// Отказ — не «ничего не случилось»: воркер не тронут, но след обязан лечь с именем
/// состояния и причиной. Пустой поток сделал бы запрет неотличимым от прогона без
/// решения, а `Succeeded` — от состоявшегося вызова.
#[tokio::test]
async fn a_refused_call_spawns_nothing_and_leaves_a_named_refusal() {
    let store = store();
    let worker = FakeWorker::new();
    let attempt = refused_attempt();

    let got = run_once(&attempt, &worker, &store).await.expect("отказ");
    let CallVerdict::Deny { reason } = &got else {
        panic!("ожидался Deny, а не {got:?}");
    };
    // Пустая причина прошла бы `contains("")`: отказ без причины не расследуется.
    assert!(!reason.trim().is_empty(), "отказ обязан назвать причину");
    assert_eq!(worker.spawned(), 0, "запрещённый вызов не запускает воркер");

    let seen = store.list_by_task("t-v02").await.expect("выборка");
    assert_eq!(seen.len(), 1, "ровно один чек на один вызов: {seen:?}");
    let receipt = &seen[0];
    assert_eq!(receipt.event_type, "run.call", "это чек вызова");
    assert_eq!(receipt.actor, "policy", "решение приняла политика");
    assert_eq!(
        receipt.payload.get("status").map(String::as_str),
        Some("Denied"),
        "состояние названо, а не оставлено пустым: {receipt:?}"
    );
    assert_eq!(
        receipt.payload.get("reason").map(String::as_str),
        Some(reason.as_str()),
        "причина в чеке дословно"
    );
    assert_eq!(
        receipt.risk,
        EventRisk::Classified(RiskClass::R5),
        "риск отказа — тот, что решил политику"
    );
    assert!(
        !text_of(receipt).contains("Succeeded") && !text_of(receipt).contains("Allow"),
        "отказ не записан как разрешение: {receipt:?}"
    );
}

/// Скриптованный ход модели. Очередь пуста — не отказ прогона: агент мог закончить.
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
        let tools = request.tools.as_ref().expect("набор инструментов объявлен");
        assert!(
            !tools.iter().any(|tool| tool.name == "bash"),
            "bash удерживается, но не предлагается модели: {:?}",
            tools
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>()
        );
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

fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("sw-v02-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("каталог пробега");
    root
}

fn repo(root: &Path) -> PathBuf {
    let repo = root.join("repo");
    std::fs::create_dir_all(repo.join(".git")).expect(".git");
    std::fs::write(repo.join(".git/HEAD"), "ref: refs/heads/main\n").expect("HEAD");
    std::fs::create_dir_all(repo.join("src")).expect("src");
    std::fs::write(repo.join("src/lib.rs"), "pub fn one() -> u8 { 1 }\n").expect("lib.rs");
    repo
}

/// Сборка под настоящим Pi: модель просит `bash`, политика держит его как R5. Если
/// заявка доходит до гейта, отказ обязан быть назван, а кандидат — пуст. Если хук не
/// срабатывает, прогон не вправе выглядеть как состоявшийся без решения.
#[test]
fn a_forbidden_build_call_is_denied_and_leaves_no_candidate() {
    let root = scratch("build");
    let repo = repo(&root);
    let call = Frame::ToolCall {
        id: "call-1".to_string(),
        name: "bash".to_string(),
        input: json!({ "command": "echo pwn" }),
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
        outcome.gate.iter().any(|answer| matches!(
            answer,
            GateAnswer::Deny { tool, reason } if tool == "bash" && reason.contains("R5")
        )),
        "гейт обязан назвать отказ bash: gate={:?} requests={} turns={} candidate={:?} output={}",
        outcome.gate,
        outcome.gate_requests,
        outcome.model_turns,
        outcome.candidate,
        outcome.output
    );
    assert!(
        outcome.gate_requests > 0,
        "вызов обязан дойти до гейта: gate={:?} requests={} turns={} candidate={:?} output={}",
        outcome.gate,
        outcome.gate_requests,
        outcome.model_turns,
        outcome.candidate,
        outcome.output
    );
    assert!(
        !outcome
            .gate
            .iter()
            .any(|answer| matches!(answer, GateAnswer::Allow { .. })),
        "гейт не разрешил bash: gate={:?}",
        outcome.gate
    );
    assert!(
        !outcome.output.contains("Succeeded"),
        "отказ не записан как успех: {}",
        outcome.output
    );
    assert!(
        outcome.candidate.is_empty(),
        "запрещённый вызов не оставляет кандидата: {:?}",
        outcome.candidate
    );
}
