//! RED M4/packet-05: решение обязано дойти до того, что ПРОИЗОЙДЁТ: запрещённый вызов не
//! состоялся, но виден в расследовании. Ожидаемое решение берётся из самого `authorize`.

use std::cell::Cell;
use std::future::{ready, Future};
use std::path::Path;
use std::pin::Pin;

use stellwerk_core::contract::{
    CommitSha, DataClass, Event, EventRisk, ModelBinding, PolicyDigest, PolicyLevel, RecipeDigest,
    RetentionClass, RiskClass, TaskStatus,
};
use stellwerk_gateway::permission::{PermissionError, ToolRisk};
use stellwerk_store::{EventStore, SqliteStore, TaskStore};
use stellwerk_task::runner::{
    authorize, run_once, Call, CallAttempt, CallVerdict, ExecOutcome, RiskRegistry, RunOutcome,
    StopReason, Verdict, Worker, WorkerError,
};
use stellwerk_task::taskfile::TaskFile;

/// Счётчик запусков отделён от факта записи: иначе «воркер не вызван» прошёл бы и при
/// молчащем раннере.
struct FakeWorker {
    launches: Cell<usize>,
    fail: bool,
    registry: TestRegistry,
}

impl FakeWorker {
    fn new(fail: bool) -> Self {
        Self {
            launches: Cell::new(0),
            fail,
            registry: TestRegistry::new(),
        }
    }
}

/// Доверенный реестр для вертикали: класс инструмента приходит ОТСЮДА, а не из
/// `call.risk`. `bash`/`fs` — чтение R0 (его имя держит дайджест тела), `r2`/`r5`
/// дают классы для отказов, `unclass` — отсутствие классификации.
struct TestRegistry {
    entries: Vec<(String, String, ToolRisk)>,
}

impl TestRegistry {
    fn new() -> Self {
        Self {
            entries: vec![
                ("bash".into(), "fs".into(), R0),
                ("r2".into(), "fs".into(), R2),
                ("r5".into(), "fs".into(), R5),
                ("unclass".into(), "fs".into(), ToolRisk::Unclassified),
            ],
        }
    }
}

impl RiskRegistry for TestRegistry {
    fn risk_of(&self, tool: &str, module: &str) -> Result<ToolRisk, PermissionError> {
        self.entries
            .iter()
            .find(|(t, m, _)| t == tool && m == module)
            .map(|(_, _, risk)| *risk)
            .ok_or(PermissionError::TableIncomplete {
                risk: RiskClass::R0,
            })
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

    fn registry(&self) -> &dyn RiskRegistry {
        &self.registry
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
    // Инструмент выбирается по классу в реестре: класс берётся из реестра, а не из
    // `call.risk`, и один инструмент не может нести два разных класса.
    let (tool, module) = match risk {
        ToolRisk::Unclassified => ("unclass", "fs"),
        ToolRisk::Classified(RiskClass::R2) => ("r2", "fs"),
        ToolRisk::Classified(RiskClass::R5) => ("r5", "fs"),
        _ => ("bash", "fs"),
    };
    CallAttempt {
        task_id: task.into(),
        call: Call {
            tool: tool.into(),
            module: module.into(),
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

/// Чек вызова — только `run.call`; карантин несёт ту же причину отдельной записью.
fn carries_call(events: &[Event], needle: &str) -> usize {
    events
        .iter()
        .filter(|e| e.event_type == "run.call" && text_of(e).contains(needle))
        .count()
}

fn quarantines(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|e| e.event_type == "run.quarantine")
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
    assert_eq!(
        got,
        authorize(&ask.call, ask.caller, worker.registry()).verdict,
        "не наш вердикт"
    );
    assert!(matches!(got, CallVerdict::Deny { .. }), "{got:?}");
    assert_eq!(worker.spawned(), 0, "запускать нельзя");
    assert!(!trail(&store, "t-refused").await.is_empty(), "нет следа");
}

/// Причина обязана дойти дословно и с кодом правила.
#[tokio::test]
async fn the_denial_reason_reaches_the_trail_with_its_rule_code() {
    let (store, worker) = bench(false);
    let ask = attempt("t-reason", R5, true, PolicyLevel::L5Owner);
    let reason = deny_reason(authorize(&ask.call, ask.caller, worker.registry()).verdict);
    run_once(&ask, &worker, &store).await.expect("отказ");
    let seen = trail(&store, "t-reason").await;
    let code = reason.split(':').next().unwrap_or_default();
    // Без этой проверки `contains("")` прошёл бы на пустой причине.
    assert!(!reason.trim().is_empty() && !code.is_empty(), "нет кода");
    assert_eq!(carries_call(&seen, &reason), 1, "причина потерялась");
    assert_eq!(carries_call(&seen, code), 1, "код правила не в потоке");
    assert_eq!(quarantines(&seen), 1, "карантин — отдельная запись");
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
        // Именно `Unclassified`, а не «не R0»: `assert_ne!` прошёл бы и при R1, то есть при
        // том самом схлопывании, которое пакет и устраняет. Неизвестное обязано быть названо
        // своим состоянием, иначе «нет данных» снова неотличимо от «риск низкий».
        assert_eq!(
            e.risk,
            EventRisk::Unclassified,
            "неизвестный риск обязан остаться Unclassified, а не стать классом",
        );
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
    assert_eq!(carries_call(&seen, &deny_reason(bad)), 1, "причина одна");
    assert_eq!(quarantines(&seen), 1, "карантин — отдельная запись");
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
    assert_eq!(carries_call(&seen, &deny_reason(bad)), 1, "причина одна");
    assert_eq!(quarantines(&seen), 1, "карантин — отдельная запись");
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

/// Пакет 06: «в чеке только хеш, никогда тело вывода» — восемь сценариев выше не ловят.
#[tokio::test]
async fn a_receipt_carries_the_digest_of_the_output_and_never_its_body() {
    let (store, worker) = bench(false);
    let ask = attempt("t-body", R0, true, PolicyLevel::L2Contributor);
    run_once(&ask, &worker, &store).await.expect("разрешён");
    let seen = trail(&store, "t-body").await;
    // Дайджест зашит, а не посчитан: это SHA-256 тела `format!("вывод {}", "bash")` — смена алгоритма ломает тест, а не смысл чеков.
    assert_eq!(carries(&seen, BODY_SHA256), 1, "в потоке только дайджест");
    assert_eq!(carries(&seen, "вывод"), 0, "тело вывода в поток не уходит");
}

const BODY_SHA256: &str = "886400b989d41df6ea6ca1de6958a25d77565fe6e1ba994e84d5c9920e9c06b0";

// ═══════════════════════════════════════════════════════════════════════════
// M4/packet-08 — вертикальный срез: один Task доходит до `Done`
// ═══════════════════════════════════════════════════════════════════════════

/// Фикстура `m4-vertical-demo` пишется во временный каталог, а не лежит в репозитории:
/// `files_allowed` пакета не содержит ни одного `.toml`, и `TaskFile::load` требует путь.
const DEMO_FIXTURE: &str = r#"
[task]
id = "m4-vertical-demo"
goal = "Дойти до Done с проверенным доказательством"
workspace_ref = "ws-stellwerk"
model_ref = "qwen-local"
profile = "pi"
packet = "выполнить ровно один разрешённый вызов"
evidence_policy = "cargo test -p stellwerk-task --test vertical"

[task.limits]
max_files = 3
max_diff_lines = 200
timeout_seconds = 900
max_retries = 2
context_budget_tokens = 40000
data_class = "internal"
"#;

fn demo_fixture() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("stellwerk-m4-08");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("fixture dir must be creatable");
    let path = dir.join("m4-vertical-demo.toml");
    std::fs::write(&path, DEMO_FIXTURE).expect("fixture must be written");
    path
}

/// `TaskStore` отдаёт future, а проверка статусов синхронна: тот же `rt()`, что и в
/// `lifecycle.rs`, — из async-контекста `block_on` не вызывается.
fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Runtime::new().expect("runtime must build")
}

/// `advance` создаёт `Runtime` и зовёт `block_on`, поэтому цепочка статусов проверяется
/// вне async-контекста: внутри `#[tokio::test]` `block_on` запаникует.
#[test]
fn happy_path_reaches_done_with_verified_attestation() {
    use stellwerk_task::lifecycle::advance;

    let loaded = TaskFile::load(&demo_fixture()).expect("фикстура обязана читаться");
    assert_eq!(loaded.spec.id, "m4-vertical-demo", "id из фикстуры");

    let store = store();
    let spec = loaded
        .into_spec()
        .expect("фикстура обязана давать TaskSpec");
    let id = spec.id.clone();
    rt().block_on(store.create(spec))
        .expect("create must succeed");

    // Каждый переход сначала проверяется предикатом, потом выполняется: иначе «дошёл до
    // Done» ничего не говорит о том, каким путём дошёл.
    let path = [
        TaskStatus::Planned,
        TaskStatus::Reviewed,
        TaskStatus::Delegated,
        TaskStatus::Running,
        TaskStatus::EvidenceSubmitted,
        TaskStatus::Verified,
        TaskStatus::Validated,
        TaskStatus::Done,
    ];
    let mut current = TaskStatus::Draft;
    for next in path {
        assert!(
            current.can_transition_to(next),
            "{current:?} → {next:?} обязан быть разрешён предикатом"
        );
        let landed = advance(&store, &id, next).expect("переход обязан пройти");
        assert_eq!(landed, next);
        current = landed;
    }
    assert_eq!(
        rt().block_on(store.status_of(&id))
            .expect("read must succeed"),
        Some(TaskStatus::Done),
        "задача обязана дойти до Done"
    );
}

/// `evaluate` требует `ToolAttestation` **и** `Validation`, иначе гейт не закрывается.
/// Плюс вердикт прогона `Valid`: гейт читает его как данное и не пересчитывает.
#[test]
fn the_run_gate_closes_only_with_attestation_and_validation() {
    use stellwerk_task::evidence::{evaluate, EvidenceEntry, EvidenceKind};

    let outcome = RunOutcome {
        stop_reason: StopReason::Stop,
        tool_steps: 1,
        turns: 1,
        total_input_tokens: 120,
        verdict: Verdict::Valid,
    };
    let entry = |id: &str, kind: EvidenceKind| {
        let mut entry = EvidenceEntry {
            id: id.to_owned(),
            task_id: "m4-vertical-demo".to_owned(),
            kind,
            command: "cargo test -p stellwerk-task --test vertical".to_owned(),
            result_hash: format!("hash-{id}"),
            ts: "2026-10-01T00:00:00Z".to_owned(),
            recipe_digest: None,
            policy_digest: None,
            model_fingerprint: None,
            commit_sha: None,
        };
        if kind == EvidenceKind::ToolAttestation {
            entry.recipe_digest = Some(RecipeDigest("sha256:vertical-recipe".to_owned()));
            entry.policy_digest = Some(PolicyDigest("sha256:vertical-policy".to_owned()));
            entry.commit_sha = Some(CommitSha("vertical-commit".to_owned()));
        }
        entry
    };

    let closed = evaluate(
        &outcome,
        &[
            entry("e-1", EvidenceKind::ModelClaim),
            entry("e-2", EvidenceKind::ToolAttestation),
            entry("e-3", EvidenceKind::Validation),
        ],
    );
    assert!(
        closed.passed,
        "полная цепочка обязана закрывать гейт: {:?}",
        closed.missing
    );

    let without_attestation = evaluate(
        &outcome,
        &[
            entry("e-1", EvidenceKind::ModelClaim),
            entry("e-3", EvidenceKind::Validation),
        ],
    );
    assert!(
        !without_attestation.passed,
        "без ToolAttestation гейт обязан остаться открытым"
    );

    // Непустая аттестация: у разрешённого вызова есть хеш результата, и это отличие от
    // вызова отказного, где аттестации нет вовсе.
    let att = entry("e-2", EvidenceKind::ToolAttestation);
    assert!(
        !att.result_hash.is_empty(),
        "аттестация обязана нести хеш результата"
    );
}

/// Негативный сосед. Пакет утверждал, что он уже есть в packet-05 — измерением он не
/// найден нигде, кроме текстов пакетов, значит писать его здесь.
#[tokio::test]
async fn denied_call_leaves_no_attestation() {
    let (store, worker) = bench(false);
    let ask = attempt("t-denied", R5, true, PolicyLevel::L5Owner);
    let got = run_once(&ask, &worker, &store).await.expect("отказ");
    assert!(
        matches!(got, CallVerdict::Deny { .. }),
        "R5 у владельца — отказ"
    );
    assert_eq!(worker.spawned(), 0, "отказный вызов не запускает воркер");

    let seen = trail(&store, "t-denied").await;
    assert!(!seen.is_empty(), "отказ тоже виден");
    let blob: String = seen.iter().map(text_of).collect();
    assert!(
        !blob.contains("ToolAttestation"),
        "у отказного вызова аттестации быть не должно — её нечего подтверждать"
    );
}
