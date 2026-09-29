//! Контракт сессии (M3-11a RED): СОСТОЯНИЯ, а не сценарии вызова — маршрутизация в 11b,
//! вопросы и сбои в 11c, реализация в 12. Модуль берётся публичным API, а не `#[path]`:
//! тот собрал бы вторую копию и оставил экспорт непроверенным (F-14).

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Mutex;

use stellwerk_core::contract::{DataClass, ModelBinding, PolicyLevel, RetentionClass, RiskClass};
use stellwerk_gateway::gate::{CallRequest, Verdict};
use stellwerk_gateway::mount::MountPlan;
use stellwerk_gateway::permission::ToolRisk;
use stellwerk_gateway::receipt::Receipt;
use stellwerk_gateway::session::{
    drive, ApprovalRequest, CallOutcome, Session, SessionError, ToolBackend, ToolError,
};

/// Сессия-донор: поля тест не проверяет, заполняются правдоподобно.
fn session() -> Session {
    Session {
        id: "run-11a".into(),
        plan: MountPlan {
            mounted: vec!["read_file".into()],
            skipped: Vec::new(),
        },
        caller: PolicyLevel::L5Owner,
        budget_tokens: 43_000,
        token_calibration: None,
        calls: 0,
        subagent_depth: 0,
        root: std::env::temp_dir().join("m3-11a"),
    }
}

/// Запрос-донор: разбирается в 11b и 11c, здесь нужен лишь как вход в шов.
fn request() -> CallRequest {
    CallRequest {
        tool: "read_file".into(),
        module: "reg".into(),
        risk: ToolRisk::Classified(RiskClass::R0),
        caller: PolicyLevel::L5Owner,
        data: DataClass::Public,
        model: ModelBinding {
            id: "b".into(),
            provider: "neuraldeep".into(),
            model_id: "m".into(),
            data_class: DataClass::Public,
            retention: RetentionClass::FirstParty,
            context_window: 200_000,
            max_output: 8_000,
            budget_tokens_per_task: 43_000,
        },
        interactive: false,
    }
}

/// Подделка шва: считает вызовы и отдаёт заранее заданный ответ. Считать обязаны —
/// «инструмент не звали» недоказуемо, если не видно, что не позвали.
struct Fake {
    /// Кому передали вызов: пусто — не звали ни разу.
    invoked: Mutex<Vec<String>>,
    /// Ответ инструмента: `Err` — упал.
    reply: Result<String, ToolError>,
}

impl ToolBackend for Fake {
    /// Пометить вызов и отдать заранее заданное.
    fn invoke(
        &self,
        tool: &str,
        _input: &str,
    ) -> Pin<Box<dyn Future<Output = Result<String, ToolError>> + Send + '_>> {
        let invoked = &self.invoked;
        let (tool, reply) = (tool.to_owned(), self.reply.clone());
        Box::pin(async move {
            invoked.lock().expect("мьютекс цел").push(tool);
            reply
        })
    }
}

#[test]
fn a_failed_tool_keeps_its_receipt_inside_the_error() {
    // Дано: инструмент упал и чек вызова записан. Тогда: чек доступен ПОСЛЕ ошибки.
    // При `Result<CallOutcome, _>` он иначе исчез бы, а вызов без следа неотличим от
    // вызова, которого не было.
    let err = SessionError::Tool {
        source: ToolError("сбой чужого клиента".into()),
        receipt: Box::new(Receipt {
            call_id: "call-1".into(),
            tool: "read_file".into(),
            module: "reg".into(),
            verdict: Verdict::Allow,
            risk: RiskClass::R0,
            input_hash: "a".repeat(64),
            output_hash: "b".repeat(64),
            truncated: false,
            duration_ms: 3,
            ts: "2026-09-29T00:00:00Z".into(),
        }),
    };
    match err {
        SessionError::Tool { source, receipt } => {
            let want = ToolError("сбой чужого клиента".into());
            assert_eq!(source, want, "чужой текст цел");
            assert_eq!(receipt.call_id, "call-1", "чек доехал");
            assert_eq!(receipt.input_hash.len(), 64, "чек заполнен");
        }
        other => panic!("ожидался SessionError::Tool, получено {other:?}"),
    }
}

#[test]
fn an_expired_question_is_not_a_policy_denial() {
    // Дано: истёкший вопрос и запрет по политике. Тогда: это РАЗНЫЕ состояния — разные
    // варианты и разный путь. Отказ приезжает в `Ok`, истечение в `Err`: слить их
    // значило бы назвать отказом то, о чём не отказывали (В4).
    let timeout = SessionError::ApprovalTimeout;
    let denied = Verdict::Deny {
        reason: "risk_denied: R5".into(),
    };
    assert!(
        matches!(&timeout, SessionError::ApprovalTimeout),
        "свой вариант"
    );
    assert_ne!(
        format!("{timeout:?}"),
        format!("{denied:?}"),
        "разны в evidence"
    );
    let refused: Result<CallOutcome, SessionError> = Ok(CallOutcome {
        verdict: denied,
        shaped: None,
        receipt: None,
    });
    assert!(refused.is_ok(), "запрет политикой — не сбой сессии");
}

#[test]
fn the_question_survives_the_inbox_round_trip() {
    // Дано: вопрос, как он ляжет в inbox. Тогда: читается обратно без искажения, и
    // `args_hash` остаётся sha256 — подтверждаются те же аргументы, что и виделись.
    let asked = ApprovalRequest {
        run_id: "run-11a".into(),
        tool: "read_file".into(),
        risk: ToolRisk::Classified(RiskClass::R2),
        args_hash: "ab".repeat(32),
        requested_at: "2026-09-29T00:00:00Z".into(),
        expires_at: "2026-09-29T04:00:00Z".into(),
    };
    let wire = serde_json::to_value(&asked).expect("вопрос сериализуем");
    let back: ApprovalRequest = serde_json::from_value(wire.clone()).expect("читается");
    assert_eq!(back, asked, "круг не исказил вопрос");
    assert_eq!(wire["args_hash"], asked.args_hash, "хеш цел");
    assert_eq!(wire["risk"], "R2", "риск на проводе — словом класса");
    // Неизвестный риск переживает круг СВОИМ состоянием, а не вырождается в R0.
    let blind = ApprovalRequest {
        risk: ToolRisk::Unclassified,
        ..asked
    };
    let blind_wire = serde_json::to_value(&blind).expect("сериализуем");
    let back: ApprovalRequest = serde_json::from_value(blind_wire).expect("читается");
    assert_eq!(back.risk, ToolRisk::Unclassified, "неизвестный ≠ R0");
}

#[test]
fn a_new_session_keeps_no_subagents_and_has_where_to_ask() {
    // Дано: новая сессия. Тогда: глубина субагентов НАБЛЮДАЕМА и равна 0 (В3) — иначе
    // правило через полгода не отличить от его отсутствия. `root` отвечает на вопрос
    // «куда пишется .stellwerk/inbox/<run_id>.json», иначе вопросу некуда лечь.
    let s = session();
    assert_eq!(s.subagent_depth, 0, "В3 держит глубину");
    assert_eq!(s.calls, 0, "невидимый вызов не считается");
    let question: PathBuf = s.root.join(format!(".stellwerk/inbox/{}.json", s.id));
    assert!(question.starts_with(&s.root), "вопрос под корнем сессии");
}

#[test]
fn the_seam_is_reachable_and_the_test_drives_the_future_itself() {
    // Дано: подделка шва вместо живого процесса — gateway инструменты не выполняет.
    // Тогда: `handle` ПРИНИМАЕТ подделку и отдаёт future, который `drive` доводит без
    // чужого исполнителя: крейт от `tokio` не зависит, а опрос объявлен в `session.rs`
    // для 11b и 11c. Какой вердикт придёт — решает packet-12; здесь шов, а не
    // поведение, и до packet-12 тест падает ровно на заглушке `handle`.
    let mut s = session();
    let fake = Fake {
        invoked: Mutex::new(Vec::new()),
        reply: Ok("вывод".into()),
    };
    let outcome = drive(s.handle(&request(), &fake));
    // След обязан быть в ЛЮБОМ исходе: чек — либо в успехе, либо внутри ошибки. Истёкший
    // вопрос следа не требует: инструмент не звали, и звать его было нельзя.
    let traced = match &outcome {
        Ok(outcome) => outcome.receipt.is_some(),
        Err(SessionError::Tool { receipt, .. }) => !receipt.call_id.is_empty(),
        Err(SessionError::ApprovalTimeout) => true,
    };
    assert!(traced, "вызов без следа: {outcome:?}");
}
