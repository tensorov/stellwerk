//! V1/packet-v04: S-цепочка (приостановка → человеческая валидация →
//! возобновление → опровержение) с evidence и отличимым статусом на каждом шаге.
//! Шаг без записи не проходит: у перехода есть отрицательный контроль.
//! `Refuted` выносится из `EvidenceSubmitted`, а не из `HumanValidated` (ADR-0022).

use std::{
    cell::Cell,
    future::{ready, Future},
    path::Path,
    pin::Pin,
};

use stellwerk_core::contract::{
    CommitSha, DataClass, Event, ModelBinding, PolicyDigest, PolicyLevel, RecipeDigest,
    RetentionClass, RiskClass, TaskStatus,
};
use stellwerk_gateway::permission::ToolRisk;
use stellwerk_store::{EventStore, SqliteStore};
use stellwerk_task::{
    approval::{next_status, StatusEffect},
    evidence::{
        evaluate, evidence_status, model_claim, refutation, tool_attestation, validation,
        EvidenceEntry, EvidenceLedger, EvidenceStatus, RefutationError, FIXTURE_VALIDATION_LABEL,
    },
    ledger_sqlite::SqliteEvidenceLedger,
    orchestrator::PRODUCTION_REGISTRY,
    runner::{
        run_once, Call, CallAttempt, CallVerdict, ExecOutcome, RiskRegistry, RunOutcome,
        StopReason, Verdict, Worker, WorkerError,
    },
};

const TASK: &str = "v04-s-chain";
const TS: &str = "2026-11-03T00:00:00Z";
const CMD: &str = "cargo test -p stellwerk-cli --test vertical_v4";
const RESULT: &str = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
const HUMAN: &str = "owner reviewed the incident and accepted the result";
const BASIS: &str = "the resumed run contradicts the attested digest";
const R3: ToolRisk = ToolRisk::Classified(RiskClass::R3);

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Runtime::new().expect("runtime must build")
}

fn store() -> SqliteStore {
    SqliteStore::open(Path::new(":memory:")).expect("store must open")
}

fn receipts(rt: &tokio::runtime::Runtime, s: &SqliteStore, id: &str) -> Vec<Event> {
    rt.block_on(s.list_by_task(id)).expect("event stream")
}

fn outcome() -> RunOutcome {
    RunOutcome {
        stop_reason: StopReason::Stop,
        tool_steps: 1,
        turns: 1,
        total_input_tokens: 10,
        verdict: Verdict::Valid,
    }
}

fn binding() -> ModelBinding {
    ModelBinding {
        id: "b-v04".into(),
        provider: "neuraldeep".into(),
        model_id: "m-v04".into(),
        data_class: DataClass::Public,
        retention: RetentionClass::FirstParty,
        context_window: 200_000,
        max_output: 8_000,
        budget_tokens_per_task: 43_000,
    }
}

fn attempt(task: &str, tool: &str, interactive: bool, caller: PolicyLevel) -> CallAttempt {
    CallAttempt {
        task_id: task.into(),
        call: Call {
            tool: tool.into(),
            module: "polyforge".into(),
            risk: R3,
            data: DataClass::Public,
            model: binding(),
            interactive,
        },
        caller,
        workspace_ref: "ws-v04".into(),
    }
}

type Exec<'a> = Pin<Box<dyn Future<Output = Result<ExecOutcome, WorkerError>> + Send + 'a>>;

struct FakeWorker(Cell<usize>);

impl Worker for FakeWorker {
    fn exec(&self, _ws: &str, call: &Call) -> Exec<'_> {
        self.0.set(self.0.get() + 1);
        Box::pin(ready(Ok(ExecOutcome {
            output: format!("вывод {}", call.tool),
        })))
    }
    fn spawned(&self) -> usize {
        self.0.get()
    }
    fn registry(&self) -> &dyn RiskRegistry {
        &PRODUCTION_REGISTRY
    }
}

fn attested(task: &str, id: &str) -> EvidenceEntry {
    let mut e = tool_attestation(task, id, CMD, RESULT, TS);
    e.recipe_digest = Some(RecipeDigest("recipe-v04".into()));
    e.policy_digest = Some(PolicyDigest("policy-v04".into()));
    e.commit_sha = Some(CommitSha("0123456789abcdef0123456789abcdef01234567".into()));
    e
}

#[test]
fn suspension_validation_refutation_chain() {
    let rt = rt();
    let s = store();
    let mut ledger = SqliteEvidenceLedger::open(Path::new(":memory:")).expect("ledger");
    let worker = FakeWorker(Cell::new(0));

    // 1. Приостановка: некому ответить — записано, воркер не запущен.
    let verdict = rt
        .block_on(run_once(
            &attempt(TASK, "evidence_verify", false, PolicyLevel::L5Owner),
            &worker,
            &s,
        ))
        .expect("suspended run");
    assert_eq!(verdict, CallVerdict::Suspended);
    assert_eq!(worker.spawned(), 0);
    let suspended = receipts(&rt, &s, TASK)
        .into_iter()
        .find(|e| e.event_type == "run.call")
        .expect("suspension receipt");
    assert_eq!(
        suspended.payload.get("status").map(String::as_str),
        Some("Suspended")
    );
    assert!(!suspended.payload["reason"].trim().is_empty());
    assert!(matches!(
        next_status(&verdict, R3),
        StatusEffect::Transition(TaskStatus::Suspended)
    ));
    assert!(TaskStatus::Running.can_transition_to(TaskStatus::Suspended));
    assert!(TaskStatus::Suspended.can_transition_to(TaskStatus::Running));
    assert!(!TaskStatus::Suspended.is_terminal());
    assert_ne!(TaskStatus::Suspended, TaskStatus::AwaitingHumanValidation);

    // 2. Валидация: удостоверение и фикстура — не решение человека.
    assert!(TaskStatus::Suspended.can_transition_to(TaskStatus::AwaitingHumanValidation));
    let claim = model_claim(TASK, "v04-claim", RESULT, TS);
    let att = attested(TASK, "v04-attest");
    let before = vec![claim.clone(), att.clone()];
    assert_eq!(
        evidence_status(&before),
        EvidenceStatus::AwaitingHumanValidation
    );
    ledger.append(&claim).expect("claim");
    ledger.append(&att).expect("attestation");
    let fixture = vec![
        claim.clone(),
        att.clone(),
        validation(TASK, "v04-fixture", FIXTURE_VALIDATION_LABEL, RESULT, TS),
    ];
    assert_eq!(
        evidence_status(&fixture),
        EvidenceStatus::AwaitingHumanValidation,
        "fixture is not a human decision"
    );
    ledger
        .append(&validation(TASK, "v04-human", HUMAN, RESULT, TS))
        .expect("human validation");
    let chain = ledger.entries(TASK);
    assert_eq!(evidence_status(&chain), EvidenceStatus::HumanValidated);
    let gate = evaluate(&outcome(), &chain);
    assert!(gate.passed, "validated chain must pass: {:?}", gate.missing);
    assert!(TaskStatus::AwaitingHumanValidation.can_transition_to(TaskStatus::HumanValidated));
    assert!(TaskStatus::HumanValidated.is_terminal());
    assert_ne!(TaskStatus::HumanValidated, TaskStatus::Refuted);

    // 3. Возобновление: без записи о решении не разрешено; с ней — доходит до воркера.
    assert_ne!(
        evidence_status(&before),
        EvidenceStatus::HumanValidated,
        "silence must not resume"
    );
    let n = receipts(&rt, &s, TASK).len();
    let spawned = worker.spawned();
    assert_eq!(
        rt.block_on(run_once(
            &attempt(TASK, "gate_report", true, PolicyLevel::L2Contributor),
            &worker,
            &s,
        ))
        .expect("resumed run"),
        CallVerdict::Allow
    );
    assert_eq!(worker.spawned(), spawned + 1, "resume reached the worker");
    let after = receipts(&rt, &s, TASK);
    assert_eq!(after.len(), n + 1, "resume left its own receipt");
    let resumed = after
        .iter()
        .find(|e| {
            e.id != suspended.id && e.payload.get("status").map(String::as_str) == Some("Succeeded")
        })
        .expect("distinct Succeeded receipt");
    let data = resumed
        .payload
        .get("output_sha256")
        .expect("output digest")
        .clone();
    assert_eq!(evidence_status(&chain), EvidenceStatus::HumanValidated);

    // 4. Опровержение: основание и ссылка на данные обязательны.
    assert_eq!(refutation("", &data), Err(RefutationError::MissingBasis));
    assert_eq!(refutation("  ", &data), Err(RefutationError::MissingBasis));
    assert_eq!(
        refutation(BASIS, "  "),
        Err(RefutationError::MissingRefutingData)
    );
    let refuted = refutation(BASIS, &data).expect("basis and data");
    assert_eq!(refuted.status(), TaskStatus::Refuted);
    assert_ne!(refuted.status(), TaskStatus::HumanValidated);
    assert_eq!(refuted.refuting_data(), data);
    assert!(TaskStatus::EvidenceSubmitted.can_transition_to(TaskStatus::Refuted));
    assert!(!TaskStatus::HumanValidated.can_transition_to(TaskStatus::Refuted));
    assert!(TaskStatus::Refuted.is_terminal());
    assert!(!TaskStatus::Refuted.can_transition_to(TaskStatus::Done));
}
