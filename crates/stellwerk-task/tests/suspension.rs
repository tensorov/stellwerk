//! V1/s01: автомат приостановки.
//!
//! `Suspended -> AwaitingHumanValidation` разрешён только тогда, когда вопрос
//! человеку действительно задан. Неинтерактивный прогон не ждёт ответа, которого
//! некому дать: он возвращает `Suspended`, пишет причину и не запускает воркер.
//! Тишина не становится ни разрешением, ни ожиданием решения (закон 6).

use std::cell::Cell;
use std::future::{ready, Future};
use std::path::Path;
use std::pin::Pin;

use stellwerk_core::contract::{
    DataClass, ModelBinding, PolicyLevel, RetentionClass, RiskClass, TaskStatus,
};
use stellwerk_gateway::permission::{PermissionError, ToolRisk};
use stellwerk_store::{EventStore, SqliteStore};
use stellwerk_task::approval::{next_status, StatusEffect};
use stellwerk_task::runner::{
    run_once, suspension_status, Call, CallAttempt, CallVerdict, ExecOutcome, RiskRegistry, Worker,
    WorkerError,
};

const R3: ToolRisk = ToolRisk::Classified(RiskClass::R3);

struct FakeWorker {
    launches: Cell<usize>,
    registry: R3Registry,
}

impl FakeWorker {
    fn new() -> Self {
        Self {
            launches: Cell::new(0),
            registry: R3Registry,
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

/// R3 требует `L5Owner` и спрашивает человека. В неинтерактивном проходе вопроса
/// задать некому, поэтому гейт обязан вернуть `Suspended`, а не `Allow`.
struct R3Registry;

impl RiskRegistry for R3Registry {
    fn risk_of(&self, tool: &str, module: &str) -> Result<ToolRisk, PermissionError> {
        match (tool, module) {
            ("net", "http") => Ok(R3),
            _ => Err(PermissionError::TableIncomplete {
                risk: RiskClass::R0,
            }),
        }
    }
}

fn binding() -> ModelBinding {
    ModelBinding {
        id: "b-s01".into(),
        provider: "neuraldeep".into(),
        model_id: "m-s01".into(),
        data_class: DataClass::Public,
        retention: RetentionClass::FirstParty,
        context_window: 200_000,
        max_output: 8_000,
        budget_tokens_per_task: 43_000,
    }
}

fn attempt(task: &str, interactive: bool) -> CallAttempt {
    CallAttempt {
        task_id: task.into(),
        call: Call {
            tool: "net".into(),
            module: "http".into(),
            risk: R3,
            data: DataClass::Public,
            model: binding(),
            interactive,
        },
        caller: PolicyLevel::L5Owner,
        workspace_ref: "ws-s01".into(),
    }
}

/// Чек прогона по задаче: единственное место, где причина паузы становится
/// наблюдаемой. Отсутствие чека — провал, а не «причина не нашлась».
async fn receipt(store: &SqliteStore, task: &str) -> stellwerk_core::contract::Event {
    store
        .list_by_task(task)
        .await
        .expect("read receipts")
        .into_iter()
        .find(|event| event.event_type == "run.call")
        .unwrap_or_else(|| panic!("нет чека run.call для {task}"))
}

#[tokio::test]
async fn non_interactive_run_does_not_hang_and_does_not_allow() {
    // Given: неинтерактивный вызов R3 от владельца — гейт хотел бы спросить,
    // но канала ответа нет.
    let store = SqliteStore::open(Path::new(":memory:")).expect("store opens");
    let worker = FakeWorker::new();
    let task = "t-s01-no-question";

    // When: прогон ожидается напрямую, без таймаута, превращающего тишину в
    // разрешение. Если бы вызов ждал ответа, тест не завершился бы.
    let verdict = run_once(&attempt(task, false), &worker, &store)
        .await
        .expect("run returns");

    // Then: пауза — не разрешение и не вопрос, адресованный человеку.
    assert_eq!(verdict, CallVerdict::Suspended, "ответить некому");
    assert_ne!(verdict, CallVerdict::Allow, "молчание не разрешает вызов");
    assert_ne!(verdict, CallVerdict::Ask, "вопрос человеку не задан");
    assert_eq!(worker.spawned(), 0, "пауза не запускает воркер");

    // Then: отсутствие вопроса — отдельное состояние, а не ожидание решения.
    assert_eq!(
        suspension_status(&verdict),
        Some(TaskStatus::Suspended),
        "без вопроса приостановка остаётся паузой"
    );
    assert_ne!(
        suspension_status(&verdict),
        Some(TaskStatus::AwaitingHumanValidation),
        "тишина не выдаётся за ожидание человека"
    );
    assert_eq!(
        next_status(&verdict, R3),
        StatusEffect::Transition(TaskStatus::Suspended),
        "production-эффект: пауза, не AwaitingHumanValidation"
    );
    assert_ne!(
        next_status(&verdict, R3),
        StatusEffect::Transition(TaskStatus::AwaitingHumanValidation),
        "переход к человеку выведен из отсутствия ответа"
    );

    // Then: причина паузы записана и названа, а не оставлена пустой.
    let paused = receipt(&store, task).await;
    assert_eq!(
        paused.payload.get("status").map(String::as_str),
        Some("Suspended"),
        "чек записан не своим статусом"
    );
    let reason = paused.payload.get("reason").expect("причина паузы");
    assert!(!reason.trim().is_empty(), "причина паузы пуста");

    // Given: тот же вызов, но интерактивный — вопрос человеку существует.
    let asked_task = "t-s01-question";
    // When:
    let asked = run_once(&attempt(asked_task, true), &worker, &store)
        .await
        .expect("interactive run returns");

    // Then: вопрос задан, и только он разрешает ожидание человека.
    assert_eq!(asked, CallVerdict::Ask);
    assert_eq!(worker.spawned(), 0, "вопрос человеку не запускает воркер");
    assert_eq!(
        suspension_status(&asked),
        Some(TaskStatus::AwaitingHumanValidation),
        "вопрос человеку разрешает AwaitingHumanValidation"
    );
    assert!(
        TaskStatus::Suspended.can_transition_to(TaskStatus::AwaitingHumanValidation),
        "переход Suspended -> AwaitingHumanValidation должен быть достижим"
    );
    let asked_receipt = receipt(&store, asked_task).await;
    assert_eq!(
        asked_receipt.payload.get("status").map(String::as_str),
        Some("Asked"),
        "вопрос записан не своим статусом"
    );
    assert!(
        !asked_receipt
            .payload
            .get("reason")
            .expect("причина вопроса")
            .trim()
            .is_empty(),
        "причина вопроса пуста"
    );
}

#[test]
fn suspension_status_distinguishes_question_from_silence() {
    // Вопрос есть — ожидание человека разрешено.
    assert_eq!(
        suspension_status(&CallVerdict::Ask),
        Some(TaskStatus::AwaitingHumanValidation)
    );
    // Вопроса нет — отдельная пауза, не ожидание решения.
    assert_eq!(
        suspension_status(&CallVerdict::Suspended),
        Some(TaskStatus::Suspended)
    );
    // Вердикты, которые не приостанавливают прогон, не превращаются в паузу.
    assert_eq!(suspension_status(&CallVerdict::Allow), None);
    assert_eq!(
        suspension_status(&CallVerdict::Deny {
            reason: "denied".into()
        }),
        None
    );
}
