//! V1/o04: `Refuted` принимается только с названным основанием и ссылкой на
//! опровергающие данные.
//!
//! Опровержение без основания неотличимо от отказа без причины: пустая строка и
//! пробелы — это отсутствие основания, а не основание (закон 6). Ссылка на
//! опровергающие данные не теряется: без неё опровержение нечем сверить с
//! данными. `Refuted` — человеческое решение о предъявленных доказательствах, а
//! не вердикт о провале прогона, поэтому он не становится `Failed`.

use std::cell::Cell;
use std::future::{ready, Future};
use std::path::Path;
use std::pin::Pin;

use stellwerk_core::contract::{
    DataClass, ModelBinding, PolicyLevel, RetentionClass, RiskClass, TaskStatus,
};
use stellwerk_gateway::permission::{PermissionError, ToolRisk};
use stellwerk_store::SqliteStore;
use stellwerk_task::evidence::{
    evaluate, evidence_status, model_claim, refutation, refutation_entry, tool_attestation,
    validation, EvidenceEntry, EvidenceLedger, EvidenceStatus, LedgerError, RefutationError,
    FIXTURE_VALIDATION_LABEL,
};
use stellwerk_task::runner::{
    resume_after_human_validation, run_once_resuming, Call, CallAttempt, ExecOutcome,
    ResumeOutcome, RiskRegistry, RunOutcome, StopReason, Verdict, Worker, WorkerError,
};

const TASK: &str = "t-refutation";
/// Время фиксировано: проверяется основание опровержения, а не время записи.
const TS: &str = "2026-11-02T00:00:00Z";
const TEST_CMD: &str = "cargo test -p stellwerk-task --test refutation";
const BASIS: &str = "the attestation hash does not match the recorded artifact";
const REFUTING_DATA: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000000";

/// Удостоверенная цепочка: предъявлять есть что, решения человека ещё нет.
fn attested() -> Vec<EvidenceEntry> {
    vec![
        model_claim(TASK, "e-claim", "h-claim", TS),
        tool_attestation(TASK, "e-tool", TEST_CMD, "h-tool", TS),
    ]
}

#[test]
fn refuted_requires_stated_basis() {
    // Given/When: основание пусто. Then: опровержение отвергнуто, а не принято
    // молча — иначе оно неотличимо от отказа без причины.
    assert_eq!(
        refutation("", REFUTING_DATA),
        Err(RefutationError::MissingBasis),
        "пустое основание — это отсутствие основания, а не основание"
    );

    // When: основание состоит только из пробелов. Then: это тоже отсутствие.
    assert_eq!(
        refutation("   ", REFUTING_DATA),
        Err(RefutationError::MissingBasis),
        "пробельное основание ничего не называет"
    );

    // When: ссылка на опровергающие данные пуста или пробельна. Then: проверять
    // опровержение не по чему — отказ.
    assert_eq!(
        refutation(BASIS, ""),
        Err(RefutationError::MissingRefutingData),
        "ссылка на опровергающие данные обязательна"
    );
    assert_eq!(
        refutation(BASIS, "  "),
        Err(RefutationError::MissingRefutingData),
        "пробельная ссылка — это отсутствие ссылки"
    );

    // Given/When: названы и основание, и ссылка. Then: опровержение принято.
    let accepted = refutation(BASIS, REFUTING_DATA).expect("основание и ссылка названы");

    // Then: опровержение — `Refuted`, а не `Failed`: провал прогона и
    // человеческое опровержение доказательств — разные факты.
    assert_eq!(
        accepted.status(),
        TaskStatus::Refuted,
        "принятое опровержение переводит задачу в Refuted"
    );
    assert_ne!(
        accepted.status(),
        TaskStatus::Failed,
        "опровержение не превращается в Failed"
    );
    assert_eq!(
        accepted.basis(),
        BASIS,
        "основание сохраняется, а не выбрасывается после проверки"
    );
    assert_eq!(
        accepted.refuting_data(),
        REFUTING_DATA,
        "ссылка на опровергающие данные не теряется"
    );

    // Given: та же цепочка плюс обычная человеческая Validation.
    let mut validated = attested();
    validated.push(validation(
        TASK,
        "e-human",
        "owner reviewed the diff and accepted",
        "h-tool",
        TS,
    ));
    // Then: обычная Validation — это принятие, а не опровержение.
    assert_eq!(
        evidence_status(&validated),
        EvidenceStatus::HumanValidated,
        "человеческая Validation остаётся принятием"
    );
    assert_ne!(
        TaskStatus::HumanValidated,
        accepted.status(),
        "принятие и опровержение — разные решения"
    );
}

/// V1/s03: «опровергнуто, но завершено» невозможно. После `Refuted` нет
/// исходящих переходов, а названное основание остаётся при опровержении, а не
/// теряется при остановке цепочки.
#[test]
fn done_after_refuted_is_rejected() {
    // Given: принятое опровержение с названным основанием и ссылкой на данные.
    let accepted = refutation(BASIS, REFUTING_DATA).expect("основание и ссылка названы");

    // (a) Then: опровержение переводит задачу в `Refuted`.
    assert_eq!(
        accepted.status(),
        TaskStatus::Refuted,
        "принятое опровержение переводит задачу в Refuted"
    );

    // (b)-(d) Then: из `Refuted` нет перехода ни в завершение, ни в валидацию,
    // ни в себя. Проверяются точные цели, а не только «не Done».
    assert!(
        !TaskStatus::Refuted.can_transition_to(TaskStatus::Done),
        "опровергнуто, но завершено — запрещено"
    );
    assert!(
        !TaskStatus::Refuted.can_transition_to(TaskStatus::Validated),
        "опровергнутое доказательство нельзя валидировать"
    );
    assert!(
        !TaskStatus::Refuted.can_transition_to(TaskStatus::Refuted),
        "переход в себя запрещён и для Refuted"
    );

    // (e) Then: `Refuted` — явный конец жизненного цикла.
    assert!(
        TaskStatus::Refuted.is_terminal(),
        "Refuted терминален: после опровержения цепочка не продолжается"
    );

    // (f) Then: основание и ссылка на опровергающие данные не теряются при
    // остановке — опровержение остаётся проверяемым, а не декоративным.
    assert_eq!(
        accepted.basis(),
        BASIS,
        "основание сохраняется после остановки цепочки"
    );
    assert_eq!(
        accepted.refuting_data(),
        REFUTING_DATA,
        "ссылка на опровергающие данные сохраняется после остановки цепочки"
    );
}

// --- V2-07c: опровержение как звено цепочки (не отсутствие и не «ждёт человека») ---

/// Запись опровержения: конструктор, а не литерал `EvidenceEntry`.
fn refuted_entry(id: &str) -> EvidenceEntry {
    refutation_entry(TASK, id, BASIS, REFUTING_DATA, TS).expect("основание и ссылка названы")
}

/// Годный вердикт прогона: проверяется опровержение, а не годность прогона.
fn valid_outcome() -> RunOutcome {
    RunOutcome {
        stop_reason: StopReason::Stop,
        tool_steps: 2,
        turns: 3,
        total_input_tokens: 1_000,
        verdict: Verdict::Valid,
    }
}

#[test]
fn refuted_chain_is_refuted_not_unattested() {
    // Given: цепочка только из опровержения — удостоверения нет вовсе.
    let refuted = evidence_status(&[refuted_entry("e-refuted")]);
    // Then: «человек опроверг» отличимо от «никто не удостоверял» (закон 6).
    assert_eq!(refuted, EvidenceStatus::Refuted);
    assert_ne!(refuted, EvidenceStatus::Unattested);
    let label = EvidenceStatus::Refuted.label();
    for other in [
        EvidenceStatus::Unattested,
        EvidenceStatus::AwaitingHumanValidation,
        EvidenceStatus::HumanValidated,
    ] {
        assert_ne!(label, other.label(), "у Refuted своя метка");
    }

    // Given: опровержение поверх удостоверения и человеческой Validation.
    let mut mixed = attested();
    mixed.push(validation(
        TASK,
        "e-human",
        "owner reviewed the diff",
        "h-tool",
        TS,
    ));
    mixed.push(refuted_entry("e-refuted"));
    // Then: опровергнутая цепочка не «принята человеком».
    assert_eq!(evidence_status(&mixed), EvidenceStatus::Refuted);
}

#[test]
fn evaluate_reports_a_refutation_and_does_not_pass() {
    let gate = evaluate(&valid_outcome(), &[refuted_entry("e-refuted")]);
    assert!(!gate.passed);
    assert_eq!(gate.missing, vec![format!("refuted: {BASIS}")]);
    assert!(
        !gate
            .missing
            .iter()
            .any(|reason| reason.contains("нет ToolAttestation")),
        "опровержение — причина, а не отсутствие аттестации (закон 6)"
    );
    assert!(!gate.chain_tail_hash.is_empty(), "хвост цепочки посчитан");
}

/// Леджер с заданными записями: фильтр по задаче делает сам раннер.
struct FixedLedger {
    entries: Vec<EvidenceEntry>,
}

impl EvidenceLedger for FixedLedger {
    fn append(&mut self, _entry: &EvidenceEntry) -> Result<(), LedgerError> {
        Ok(())
    }

    fn entries(&self, _task_id: &str) -> Vec<EvidenceEntry> {
        self.entries.clone()
    }
}

#[derive(Default)]
struct AllowRegistry;

impl RiskRegistry for AllowRegistry {
    fn risk_of(&self, _tool: &str, _module: &str) -> Result<ToolRisk, PermissionError> {
        Ok(ToolRisk::Classified(RiskClass::R0))
    }
}

/// Счётный воркер: «не вызван» доказывается счётчиком, а не отсутствием чека.
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
async fn run_once_resuming_on_a_refuted_ledger_never_calls_the_worker() {
    // Given: леджер TASK содержит опровержение.
    let ledger = FixedLedger {
        entries: vec![refuted_entry("e-refuted")],
    };
    let store = SqliteStore::open(Path::new(":memory:")).expect("открыто");
    let worker = CountingWorker::default();
    assert_eq!(
        resume_after_human_validation(&ledger, TASK),
        EvidenceStatus::Refuted,
        "опровержение этой задачи видно и в возобновлении"
    );

    // When: возобновление опровергнутого прогона.
    let outcome = run_once_resuming(&allow_attempt(TASK), &worker, &store, &ledger)
        .await
        .expect("возобновление");

    // Then: это Refuted, а не «ждёт человека», и воркер не запускался.
    assert_eq!(outcome, ResumeOutcome::Refuted);
    assert_eq!(
        worker.spawned(),
        0,
        "опровергнутый прогон не перезапускается"
    );
}

#[test]
fn fixture_validation_is_not_a_human_decision() {
    let mut chain = attested();
    chain.push(validation(
        TASK,
        "e-fixture",
        FIXTURE_VALIDATION_LABEL,
        "h-tool",
        TS,
    ));
    assert_eq!(
        evidence_status(&chain),
        EvidenceStatus::AwaitingHumanValidation
    );
}

#[test]
fn validation_without_a_refutation_is_still_human_validated() {
    let mut chain = attested();
    chain.push(validation(
        TASK,
        "e-human",
        "owner reviewed the diff",
        "h-tool",
        TS,
    ));
    assert_eq!(evidence_status(&chain), EvidenceStatus::HumanValidated);
}
