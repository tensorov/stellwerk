//! Q01: отказанный прогон уходит в карантин с причиной и не становится кандидатом.
//!
//! Кандидат собирается в `stellwerk-cli/src/build_runner.rs` по байтам workspace
//! ПОСЛЕ прогона; в `stellwerk-task` такого типа нет. Поэтому «не даёт кандидата»
//! доказывается на границе раннера: отказ возвращается до `Worker::exec`, воркер
//! не запускается ни разу, и в следе задачи нет ни одного успешного чека с digest
//! вывода. Нет запуска — нет байтов, из которых CLI мог бы собрать кандидата.
//!
//! Карантин — ОТДЕЛЬНОЕ событие (`run.quarantine`), а не нормальный чек `run.call`:
//! иначе отказанный прогон был бы неотличим от состоявшегося вызова.

use std::cell::Cell;
use std::future::{ready, Future};
use std::path::Path;
use std::pin::Pin;

use stellwerk_core::contract::{DataClass, ModelBinding, PolicyLevel, RetentionClass, RiskClass};
use stellwerk_gateway::permission::{PermissionError, ToolRisk};
use stellwerk_store::{EventStore, SqliteStore};
use stellwerk_task::runner::{
    quarantine_entries, quarantine_entries_at, run_once, Call, CallAttempt, CallVerdict,
    ExecOutcome, QuarantineState, RiskRegistry, Worker, WorkerError, QUARANTINE_RETENTION_SECS,
};

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

/// R0 разрешается L2Contributor, R2 требует L3Maintainer — поэтому `r2` при
/// `L2Contributor` даёт отказ, а `r0` проходит как успешный кандидат.
struct TestRegistry;

impl RiskRegistry for TestRegistry {
    fn risk_of(&self, tool: &str, module: &str) -> Result<ToolRisk, PermissionError> {
        match (tool, module) {
            ("r0", "fs") => Ok(ToolRisk::Classified(RiskClass::R0)),
            ("r2", "fs") => Ok(ToolRisk::Classified(RiskClass::R2)),
            _ => Err(PermissionError::TableIncomplete {
                risk: RiskClass::R0,
            }),
        }
    }
}

fn holds_any_data() -> ModelBinding {
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

fn attempt(task: &str, tool: &str) -> CallAttempt {
    CallAttempt {
        task_id: task.into(),
        call: Call {
            tool: tool.into(),
            module: "fs".into(),
            risk: ToolRisk::Classified(RiskClass::R2),
            data: DataClass::Public,
            model: holds_any_data(),
            interactive: false,
        },
        caller: PolicyLevel::L2Contributor,
        workspace_ref: "ws-q01".into(),
    }
}

#[tokio::test]
async fn refused_run_is_quarantined() {
    let store = SqliteStore::open(Path::new(":memory:")).expect("открыто");
    let worker = FakeWorker::new();
    let task = "t-q01";
    let refused = attempt(task, "r2");

    let verdict = run_once(&refused, &worker, &store).await.expect("отказ");
    let CallVerdict::Deny { reason } = verdict.clone() else {
        panic!("ожидался отказ, получено {verdict:?}");
    };

    // Отказ отделён от эффекта: воркер не запускался, значит байтов для
    // кандидата после этого прогона не существует.
    assert_eq!(worker.spawned(), 0, "отказ не должен запускать воркер");

    let entries = quarantine_entries(&store, task).await.expect("карантин");
    assert_eq!(entries.len(), 1, "отказ не попал в карантин: {entries:?}");
    let entry = &entries[0];
    assert_eq!(entry.task_id, task, "карантин ушёл не той задаче");
    assert_eq!(entry.tool, "r2", "карантин потерял инструмент");
    assert!(
        !entry.reason.trim().is_empty(),
        "карантинная запись без причины"
    );
    assert_eq!(entry.reason, reason, "причина отказа изменена по дороге");

    let trail = store.list_by_task(task).await.expect("след");
    let receipt = trail
        .iter()
        .find(|event| event.event_type == "run.call")
        .expect("чек отказа пропал");
    assert_eq!(
        receipt.payload.get("status").map(String::as_str),
        Some("Denied"),
        "чек отказа потерял свой статус"
    );
    assert!(
        trail
            .iter()
            .any(|event| event.event_type == "run.quarantine"),
        "карантин слит с обычным чеком: {trail:?}"
    );

    // Кандидат не из чего собрать: отказанный прогон не оставил ни успешного
    // чека, ни digest вывода. Проверка идёт по реальному следу, а не по флагу
    // теста.
    assert!(
        trail
            .iter()
            .all(|event| event.payload.get("status").map(String::as_str) != Some("Succeeded")),
        "отказ оставил успешный чек: {trail:?}"
    );
    assert!(
        trail
            .iter()
            .all(|event| !event.payload.contains_key("output_sha256")),
        "отказ оставил digest вывода — кандидат не пуст: {trail:?}"
    );
}

#[tokio::test]
async fn allowed_run_is_not_quarantined() {
    let store = SqliteStore::open(Path::new(":memory:")).expect("открыто");
    let worker = FakeWorker::new();
    let task = "t-q01-allow";
    let allowed = attempt(task, "r0");

    let verdict = run_once(&allowed, &worker, &store)
        .await
        .expect("разрешение");
    assert_eq!(verdict, CallVerdict::Allow, "R0 должен быть разрешён");
    assert_eq!(worker.spawned(), 1, "разрешённый вызов обязан исполниться");

    let entries = quarantine_entries(&store, task).await.expect("карантин");
    assert!(
        entries.is_empty(),
        "успешный прогон попал в карантин: {entries:?}"
    );
}

#[tokio::test]
async fn expired_entry_is_marked_not_deleted() {
    let store = SqliteStore::open(Path::new(":memory:")).expect("открыто");
    let worker = FakeWorker::new();
    let task = "t-q02";
    let refused = attempt(task, "r2");

    let verdict = run_once(&refused, &worker, &store).await.expect("отказ");
    let CallVerdict::Deny { reason } = verdict else {
        panic!("ожидался отказ, получено {verdict:?}");
    };
    assert_eq!(worker.spawned(), 0, "отказ не должен запускать воркер");

    let fresh = quarantine_entries(&store, task).await.expect("карантин");
    assert_eq!(fresh.len(), 1, "отказ не попал в карантин: {fresh:?}");
    let entry = &fresh[0];
    assert!(
        entry.expires_at_secs > entry.recorded_at_secs,
        "срок хранения карантина обязан быть конечным и положительным"
    );
    assert_eq!(
        entry.expires_at_secs,
        entry.recorded_at_secs + QUARANTINE_RETENTION_SECS,
        "срок хранения не сохранён в записи"
    );
    assert_eq!(
        entry.state,
        QuarantineState::Active,
        "свежая запись обязана быть активной"
    );
    assert_eq!(entry.reason, reason, "причина отказа изменена по дороге");

    // Запись старше срока помечается истёкшей, но НЕ исчезает: читаем сразу за
    // границей срока и требуем ту же самую запись с той же причиной.
    let expired = quarantine_entries_at(&store, task, entry.expires_at_secs + 1)
        .await
        .expect("карантин");
    assert_eq!(
        expired.len(),
        1,
        "истёкшая запись исчезла без явного решения: {expired:?}"
    );
    let stale = &expired[0];
    assert_eq!(stale.state, QuarantineState::Expired, "срок не помечен");
    assert_eq!(stale.reason, reason, "истёкшая запись потеряла причину");
    assert_eq!(stale.recorded_at_secs, entry.recorded_at_secs);
    assert_eq!(stale.expires_at_secs, entry.expires_at_secs);

    // Пути удаления не было: событие карантина остаётся в append-only следе.
    let trail = store.list_by_task(task).await.expect("след");
    let quarantined = trail
        .iter()
        .filter(|event| event.event_type == "run.quarantine")
        .count();
    assert_eq!(
        quarantined, 1,
        "истёкший карантин удалён из append-only следа: {trail:?}"
    );
}
