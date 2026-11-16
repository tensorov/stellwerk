//! V1/o03: переход `AwaitingHumanValidation → HumanValidated` требует ЯВНОЙ
//! человеческой записи.
//!
//! Тишина, пустой `task_id`, пустой или пробельный `subject` — это отсутствие
//! решения, а не решение. Слить их с согласием человека значило бы выдать
//! неотличимый от разрешения дефект за приёмку (закон 6). Фикстура, помеченная
//! `FIXTURE_VALIDATION_LABEL`, решением человека не является по-прежнему.

use std::cell::Cell;
use std::future::{ready, Future};
use std::path::Path;
use std::pin::Pin;

use stellwerk_core::contract::{DataClass, ModelBinding, PolicyLevel, RetentionClass, RiskClass};
use stellwerk_gateway::permission::{PermissionError, ToolRisk};
use stellwerk_store::SqliteStore;
use stellwerk_task::evidence::{
    evidence_status, model_claim, tool_attestation, validation, EvidenceEntry, EvidenceLedger,
    EvidenceStatus, LedgerError, FIXTURE_VALIDATION_LABEL,
};
use stellwerk_task::runner::{
    resume_after_human_validation, run_once_resuming, Call, CallAttempt, ExecOutcome,
    ResumeOutcome, RiskRegistry, Worker, WorkerError,
};

const TASK: &str = "t-human-validation";
/// Другая задача: её валидация не является решением о `TASK`.
const OTHER: &str = "t-other-task";
/// Время фиксировано: решение человека проверяется, а не время его записи.
const TS: &str = "2026-11-02T00:00:00Z";
const TEST_CMD: &str = "cargo test -p stellwerk-task --test human_validation";
/// Предмет человеческого решения: кто и что решил. Кладётся в `command` записи
/// `Validation` — отдельного поля «кто проверял» у `EvidenceEntry` нет.
const HUMAN_SUBJECT: &str = "owner reviewed the diff and accepted";

/// Удостоверенная цепочка без человеческой `Validation`: предъявлять есть что,
/// но решения человека ещё нет.
fn attested() -> Vec<EvidenceEntry> {
    vec![
        model_claim(TASK, "e-claim", "h-claim", TS),
        tool_attestation(TASK, "e-tool", TEST_CMD, "h-tool", TS),
    ]
}

/// Леджер, который намеренно НЕ фильтрует по задаче: `entries` отдаёт все записи
/// на любой `task_id`. Так тест доказывает, что собственный фильтр раннера, а не
/// дисциплина хранилища, не пускает чужую валидацию в возобновление.
struct LeakyLedger {
    entries: Vec<EvidenceEntry>,
}

impl EvidenceLedger for LeakyLedger {
    fn append(&mut self, _entry: &EvidenceEntry) -> Result<(), LedgerError> {
        Ok(())
    }

    fn entries(&self, _task_id: &str) -> Vec<EvidenceEntry> {
        self.entries.clone()
    }
}

#[test]
fn resume_only_on_own_task_validation() {
    // Given: удостоверенный прогон TASK и человеческая Validation ЧУЖОЙ задачи.
    let mut foreign = attested();
    foreign.push(validation(OTHER, "e-foreign", HUMAN_SUBJECT, "h-tool", TS));
    let foreign_ledger = LeakyLedger { entries: foreign };
    assert_eq!(
        foreign_ledger.entries("unrelated").len(),
        3,
        "фейк обязан отдавать все записи, иначе тест не проверяет фильтр раннера"
    );

    // When/Then (a): валидация чужой задачи не возобновляет прогон TASK.
    assert_eq!(
        resume_after_human_validation(&foreign_ledger, TASK),
        EvidenceStatus::AwaitingHumanValidation,
        "валидация другой задачи — не решение о TASK"
    );

    // When (b): в ту же цепочку добавлена Validation самой TASK.
    let mut own = attested();
    own.push(validation(OTHER, "e-foreign", HUMAN_SUBJECT, "h-tool", TS));
    own.push(validation(TASK, "e-own", HUMAN_SUBJECT, "h-tool", TS));
    let own_ledger = LeakyLedger { entries: own };
    // Then: собственная явная запись возобновляет прогон.
    assert_eq!(
        resume_after_human_validation(&own_ledger, TASK),
        EvidenceStatus::HumanValidated,
        "Validation самой TASK возобновляет прогон"
    );

    // When (c): записи человека нет вовсе.
    let none_ledger = LeakyLedger {
        entries: attested(),
    };
    // Then: прогон ждёт человека, а не возобновляется.
    assert_eq!(
        resume_after_human_validation(&none_ledger, TASK),
        EvidenceStatus::AwaitingHumanValidation,
        "без Validation прогон ждёт человека"
    );

    // When (d): Validation без task_id.
    let mut no_task = attested();
    no_task.push(validation("", "e-no-task", HUMAN_SUBJECT, "h-tool", TS));
    let no_task_ledger = LeakyLedger { entries: no_task };
    // Then: запись не идентифицирует задачу — возобновления нет.
    assert_eq!(
        resume_after_human_validation(&no_task_ledger, TASK),
        EvidenceStatus::AwaitingHumanValidation,
        "Validation без task_id не возобновляет прогон"
    );
}

#[test]
fn transition_requires_explicit_record() {
    // Given: цепочка с удостоверением и без человеческой Validation.
    let chain = attested();
    assert_eq!(
        chain.len(),
        2,
        "фикстура состоит из утверждения и удостоверения: {chain:?}"
    );
    // When/Then: без записи человек не принял — результат ждёт человека.
    assert_eq!(
        evidence_status(&chain),
        EvidenceStatus::AwaitingHumanValidation,
        "удостоверение без решения человека не продвигает статус"
    );

    // When: добавлена человеческая Validation с непустыми task_id и subject.
    let mut validated = chain.clone();
    validated.push(validation(TASK, "e-human", HUMAN_SUBJECT, "h-tool", TS));
    // Then: только явная запись разрешает переход.
    assert_eq!(
        evidence_status(&validated),
        EvidenceStatus::HumanValidated,
        "человеческая Validation с task_id и subject — явное решение"
    );

    // When: та же Validation, но без task_id.
    let mut no_task = chain.clone();
    no_task.push(validation("", "e-no-task", HUMAN_SUBJECT, "h-tool", TS));
    // Then: запись без задачи не идентифицирует решение — переход запрещён.
    assert_eq!(
        evidence_status(&no_task),
        EvidenceStatus::AwaitingHumanValidation,
        "Validation без task_id — не человеческое решение"
    );

    // When: subject пуст.
    let mut empty_subject = chain.clone();
    empty_subject.push(validation(TASK, "e-empty", "", "h-tool", TS));
    // Then: решение не названо — это не запись, а её отсутствие.
    assert_eq!(
        evidence_status(&empty_subject),
        EvidenceStatus::AwaitingHumanValidation,
        "Validation с пустым subject ничего не решает"
    );

    // When: subject состоит только из пробелов.
    let mut whitespace_subject = chain.clone();
    whitespace_subject.push(validation(TASK, "e-space", "   ", "h-tool", TS));
    // Then: пробелы — не решение.
    assert_eq!(
        evidence_status(&whitespace_subject),
        EvidenceStatus::AwaitingHumanValidation,
        "Validation с пробельным subject ничего не решает"
    );

    // When: единственная Validation — явно помеченная фикстура.
    let mut fixture = chain;
    fixture.push(validation(
        TASK,
        "e-fixture",
        FIXTURE_VALIDATION_LABEL,
        "h-tool",
        TS,
    ));
    // Then: фикстура остаётся не-решением, даже с task_id.
    assert_eq!(
        evidence_status(&fixture),
        EvidenceStatus::AwaitingHumanValidation,
        "метка фикстуры не выдаёт тест за приёмку"
    );
}

// --- V2-03: возобновление решается статусом доказательств ДО запуска воркера ---

/// Реестр, разрешающий R0: нужен, чтобы неаттестованный путь дошёл до РЕАЛЬНОГО
/// вызова воркера, и счётчик запусков что-то доказывал.
#[derive(Default)]
struct AllowRegistry;

impl RiskRegistry for AllowRegistry {
    fn risk_of(&self, _tool: &str, _module: &str) -> Result<ToolRisk, PermissionError> {
        Ok(ToolRisk::Classified(RiskClass::R0))
    }
}

/// Счётный воркер: число запусков отделено от факта записи события, поэтому
/// «воркер не вызван» нельзя вывести из отсутствия чека.
#[derive(Default)]
struct CountingWorker {
    launches: Cell<usize>,
    registry: AllowRegistry,
}

type Exec<'a> = Pin<Box<dyn Future<Output = Result<ExecOutcome, WorkerError>> + Send + 'a>>;

impl Worker for CountingWorker {
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

fn public_binding() -> ModelBinding {
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

fn allow_attempt(task: &str) -> CallAttempt {
    CallAttempt {
        task_id: task.into(),
        call: Call {
            tool: "r0".into(),
            module: "fs".into(),
            risk: ToolRisk::Classified(RiskClass::R0),
            data: DataClass::Public,
            model: public_binding(),
            interactive: false,
        },
        caller: PolicyLevel::L2Contributor,
        workspace_ref: "ws-stellwerk".into(),
    }
}

#[tokio::test]
async fn resume_outcome_reports_own_validation_without_running_worker() {
    // Given: удостоверение и человеческая Validation самой TASK.
    let mut own = attested();
    own.push(validation(OTHER, "e-foreign", HUMAN_SUBJECT, "h-tool", TS));
    own.push(validation(TASK, "e-own", HUMAN_SUBJECT, "h-tool", TS));
    let ledger = LeakyLedger { entries: own };
    let store = SqliteStore::open(Path::new(":memory:")).expect("открыто");
    let worker = CountingWorker::default();

    // When: возобновление уже принятого прогона.
    let outcome = run_once_resuming(&allow_attempt(TASK), &worker, &store, &ledger)
        .await
        .expect("возобновление");

    // Then: прогон уже принят человеком, воркер НЕ запускается.
    assert_eq!(outcome, ResumeOutcome::AlreadyHumanValidated);
    assert_eq!(worker.spawned(), 0, "принятый прогон не перезапускается");
}

#[tokio::test]
async fn resume_outcome_foreign_validation_is_not_already_validated() {
    // Given: удостоверение TASK, но Validation принадлежит ДРУГОЙ задаче.
    let mut foreign = attested();
    foreign.push(validation(OTHER, "e-foreign", HUMAN_SUBJECT, "h-tool", TS));
    let ledger = LeakyLedger { entries: foreign };
    let store = SqliteStore::open(Path::new(":memory:")).expect("открыто");
    let worker = CountingWorker::default();

    // When: возобновление TASK.
    let outcome = run_once_resuming(&allow_attempt(TASK), &worker, &store, &ledger)
        .await
        .expect("возобновление");

    // Then: чужая валидация не возобновляет прогон — результат ждёт человека.
    assert_eq!(outcome, ResumeOutcome::AwaitingHumanValidation);
    assert_ne!(outcome, ResumeOutcome::AlreadyHumanValidated);
    assert_eq!(worker.spawned(), 0, "чужая валидация не запускает воркер");
}
