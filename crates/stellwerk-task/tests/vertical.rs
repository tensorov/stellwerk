//! RED M4/packet-05: решение обязано дойти до того, что ПРОИЗОЙДЁТ: запрещённый вызов не
//! состоялся, но виден в расследовании. Ожидаемое решение берётся из самого `authorize`.

use std::cell::Cell;
use std::future::{ready, Future};
use std::path::Path;
use std::pin::Pin;

use stellwerk_core::contract::{
    DataClass, Event, ModelBinding, PolicyLevel, RetentionClass, RiskClass,
};
use stellwerk_gateway::permission::ToolRisk;
use stellwerk_store::{EventStore, SqliteStore};
use stellwerk_task::runner::{
    authorize, run_once, Call, CallAttempt, CallVerdict, ExecOutcome, Worker, WorkerError,
};

/// Счётчик запусков отделён от факта записи: иначе «воркер не вызван» прошёл бы и при
/// молчащем раннере.
struct FakeWorker {
    launches: Cell<usize>,
    fail: bool,
}

impl FakeWorker {
    fn new(fail: bool) -> Self {
        Self {
            launches: Cell::new(0),
            fail,
        }
    }
}

type Exec<'a> = Pin<Box<dyn Future<Output = Result<ExecOutcome, WorkerError>> + Send + 'a>>;

impl Worker for FakeWorker {
    fn exec(&self, _workspace_ref: &str, call: &Call) -> Exec<'_> {
        self.launches.set(self.launches.get() + 1);
        match self.fail {
            false => Box::pin(ready(Ok(ExecOutcome {
                output: format!("вывод {}", call.tool),
            }))),
            true => Box::pin(ready(Err(WorkerError(CAUSE.to_owned())))),
        }
    }

    fn spawned(&self) -> usize {
        self.launches.get()
    }
}

const CAUSE: &str = "песочница не смонтирована";
const R0: ToolRisk = ToolRisk::Classified(RiskClass::R0);
const R2: ToolRisk = ToolRisk::Classified(RiskClass::R2);
const R5: ToolRisk = ToolRisk::Classified(RiskClass::R5);

/// Binding, пропускающий любой класс данных: иначе отказ по данным замаскирует риск.
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

fn attempt(task: &str, risk: ToolRisk, ok: bool, caller: PolicyLevel) -> CallAttempt {
    CallAttempt {
        task_id: task.into(),
        call: Call {
            tool: "bash".into(),
            module: "fs".into(),
            risk,
            data: DataClass::Public,
            model: holds_any_data(),
            interactive: ok,
        },
        caller,
        workspace_ref: "ws-stellwerk".into(),
    }
}

fn store() -> SqliteStore {
    let path = Path::new(":memory:");
    SqliteStore::open(path).expect("открыто")
}

fn bench(fail: bool) -> (SqliteStore, FakeWorker) {
    (store(), FakeWorker::new(fail))
}

async fn trail(store: &SqliteStore, task: &str) -> Vec<Event> {
    store.list_by_task(task).await.expect("выборка")
}

/// Ключи payload пакет 06 не назвал; выдумывать их здесь — заставить его повторить схему.
fn text_of(event: &Event) -> String {
    event
        .payload
        .values()
        .cloned()
        .collect::<Vec<_>>()
        .join(" | ")
}

fn carries(events: &[Event], needle: &str) -> usize {
    events
        .iter()
        .filter(|e| text_of(e).contains(needle))
        .count()
}

fn deny_reason(verdict: CallVerdict) -> String {
    let CallVerdict::Deny { reason } = verdict else {
        panic!("ожидался отказ, а не {verdict:?}");
    };
    reason
}

/// Отказ — не «ничего не случилось»: воркер не тронут, но след обязан лечь.
#[tokio::test]
async fn a_refused_call_starts_no_worker_and_still_leaves_a_receipt() {
    let (store, worker) = bench(false);
    // R2 при L2Contributor: нехватка уровня, а не запрет по риску.
    let ask = attempt("t-refused", R2, true, PolicyLevel::L2Contributor);
    let got = run_once(&ask, &worker, &store).await.expect("отказ");
    assert_eq!(got, authorize(&ask.call, ask.caller), "не наш вердикт");
    assert!(matches!(got, CallVerdict::Deny { .. }), "{got:?}");
    assert_eq!(worker.spawned(), 0, "запускать нельзя");
    assert!(!trail(&store, "t-refused").await.is_empty(), "нет следа");
}

/// Причина обязана дойти дословно и с кодом правила.
#[tokio::test]
async fn the_denial_reason_reaches_the_trail_with_its_rule_code() {
    let (store, worker) = bench(false);
    let ask = attempt("t-reason", R5, true, PolicyLevel::L5Owner);
    let reason = deny_reason(authorize(&ask.call, ask.caller));
    run_once(&ask, &worker, &store).await.expect("отказ");
    let seen = trail(&store, "t-reason").await;
    let code = reason.split(':').next().unwrap_or_default();
    // Без этой проверки `contains("")` прошёл бы на пустой причине.
    assert!(!reason.trim().is_empty() && !code.is_empty(), "нет кода");
    assert_eq!(carries(&seen, &reason), 1, "причина потерялась");
    assert_eq!(carries(&seen, code), 1, "код правила не в потоке");
}

/// Отсутствие данных — не нулевой риск: в поток не должен утечь `R0` (закон 6).
#[tokio::test]
async fn an_unclassified_tool_is_refused_and_never_looks_like_r0() {
    let (store, worker) = bench(false);
    let unknown = ToolRisk::Unclassified;
    let ask = attempt("t-unknown", unknown, false, PolicyLevel::L5Owner);
    let got = run_once(&ask, &worker, &store).await.expect("отказ");
    assert!(matches!(got, CallVerdict::Deny { .. }), "нет данных");
    assert_eq!(worker.spawned(), 0, "без класса не запускается");
    let seen = trail(&store, "t-unknown").await;
    assert!(!seen.is_empty(), "отказ по неизвестному тоже след");
    for e in &seen {
        assert_ne!(e.risk, RiskClass::R0, "«риск неизвестен» — не R0");
        assert!(!text_of(e).contains("R0"), "в payload тоже не R0");
    }
}

/// R5 не делегируется агенту ни при каком уровне.
#[tokio::test]
async fn r5_at_the_policy_owner_is_refused_and_the_worker_never_starts() {
    let (store, worker) = bench(false);
    let ask = attempt("t-r5", R5, true, PolicyLevel::L5Owner);
    let got = run_once(&ask, &worker, &store).await.expect("отказ");
    assert!(matches!(got, CallVerdict::Deny { .. }), "R5");
    assert_eq!(worker.spawned(), 0, "воркер при отказе не тронут");
    assert!(!trail(&store, "t-r5").await.is_empty(), "отказ R5 виден");
}

/// «Никто не ответил» и «запрещено политикой» — разные факты: ровно одна запись одной
/// задачи несёт причину отказа, значит вторая — приостановка, а не тот же отказ.
#[tokio::test]
async fn a_suspended_call_is_not_a_policy_refusal_in_the_trail() {
    let (store, worker) = bench(false);
    let waiting = attempt("t-v4", R2, false, PolicyLevel::L3Maintainer);
    let refused = attempt("t-v4", R2, false, PolicyLevel::L2Contributor);
    let got = run_once(&waiting, &worker, &store).await.expect("пауза");
    assert_eq!(got, CallVerdict::Suspended, "спросить некому");
    let bad = run_once(&refused, &worker, &store).await.expect("отказ");
    assert_eq!(worker.spawned(), 0, "воркера не касались");
    let seen = trail(&store, "t-v4").await;
    assert_eq!(carries(&seen, &deny_reason(bad)), 1, "причина одна");
}

/// Разрешённый вызов — единственный, ради которого воркер запускается.
#[tokio::test]
async fn an_allowed_call_starts_the_worker_exactly_once_and_leaves_a_receipt() {
    let (store, worker) = bench(false);
    let ask = attempt("t-allow", R0, true, PolicyLevel::L2Contributor);
    let got = run_once(&ask, &worker, &store).await.expect("разрешён");
    assert_eq!(got, CallVerdict::Allow, "R0 у контрибьютора");
    assert_eq!(worker.spawned(), 1, "запуск ровно один");
    assert!(!trail(&store, "t-allow").await.is_empty(), "след есть");
}

/// Отказ и состоявшийся вызов лежат в потоке ОДНОЙ задачи, и видно, чем они различны.
#[tokio::test]
async fn a_refused_attempt_and_an_executed_call_are_both_visible_in_one_task() {
    let (store, worker) = bench(false);
    let refused = attempt("t-both", R5, true, PolicyLevel::L5Owner);
    let allowed = attempt("t-both", R0, true, PolicyLevel::L2Contributor);
    let bad = run_once(&refused, &worker, &store).await.expect("отказ");
    let ok = run_once(&allowed, &worker, &store).await.expect("разрешён");
    assert_eq!(ok, CallVerdict::Allow, "R0 у контрибьютора");
    assert_eq!(worker.spawned(), 1, "запущен только разрешённый");
    let seen = trail(&store, "t-both").await;
    assert!(seen.len() >= 2, "оба вызова в потоке");
    assert_eq!(carries(&seen, &deny_reason(bad)), 1, "причина одна");
}

/// Сбой — не `Allow`: причина и статус `Failed` обязаны лечь в поток.
#[tokio::test]
async fn a_worker_failure_is_not_a_success_and_still_leaves_a_receipt() {
    let store = store();
    let ask = attempt("t-failed", R0, true, PolicyLevel::L2Contributor);
    let broken = FakeWorker::new(true);
    let result = run_once(&ask, &broken, &store).await;
    assert!(result.is_err(), "упавший вызов — не успех");
    assert_eq!(broken.spawned(), 1, "счётчик ловит запуск");
    let live = FakeWorker::new(false);
    let ok = run_once(&ask, &live, &store).await.expect("живой воркер");
    assert_eq!(ok, CallVerdict::Allow, "тот же вызов проходит");
    let seen = trail(&store, "t-failed").await;
    assert_eq!(carries(&seen, CAUSE), 1, "причина сбоя в потоке");
    assert_eq!(carries(&seen, "Failed"), 1, "статус `Failed` обязателен");
}
