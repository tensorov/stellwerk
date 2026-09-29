//! Вопрос оператору, его истечение, запрет субагентов и сбой инструмента (M3-11c RED).
//! `handle` — заглушка: четыре сценария падают на ней; маршрутизация — 11b, реализация — 12b.
//! Публичный API вместо `#[path]` (F-14) и объявленный в `session.rs` `drive`: своего писать нельзя.

use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Mutex;

use stellwerk_core::contract::{DataClass, ModelBinding, PolicyLevel, RetentionClass, RiskClass};
use stellwerk_gateway::gate::{decide, CallRequest, Verdict};
use stellwerk_gateway::mount::MountPlan;
use stellwerk_gateway::permission::ToolRisk;
use stellwerk_gateway::session::{
    drive, ApprovalRequest, Session, SessionError, ToolBackend, ToolError,
};

/// Единственный инструмент в плане: всё, чего в нём нет, вне монтажа.
const MOUNTED: &str = "read_file";

/// Подделка шва: считает и запоминает вызовы — «инструмент не звали» иначе недоказуемо.
struct Fake {
    /// Кому передали вызов, по порядку: пусто — не звали ни разу.
    invoked: Mutex<Vec<String>>,
    /// Ответ инструмента: `Err` — упал.
    reply: Result<String, ToolError>,
}

impl Fake {
    fn new(reply: Result<String, ToolError>) -> Self {
        Self {
            invoked: Mutex::new(Vec::new()),
            reply,
        }
    }

    /// Кому звонили, по порядку вызовов.
    fn seen(&self) -> Vec<String> {
        self.invoked.lock().expect("мьютекс цел").clone()
    }
}

impl ToolBackend for Fake {
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

/// Корень сессии во временном каталоге: вопрос обязан лечь на диск, а крейта `tempfile` в
/// workspace нет и `cargo add` запрещён; `create_dir_all` переживает упавший прогон.
struct Workspace(PathBuf);

impl Workspace {
    /// Каталог на время сценария; тег разводит каталоги разных сценариев.
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("sw11c-{}-{tag}", std::process::id()));
        fs::create_dir_all(&dir).expect("корень сессии создаётся");
        Self(dir)
    }

    /// Путь вопроса в inbox — раскладка объявлена в `Session::root`.
    fn question(&self, run_id: &str) -> PathBuf {
        self.0.join(format!(".stellwerk/inbox/{run_id}.json"))
    }
}

impl Drop for Workspace {
    /// Убрать за собой: рассыпанные по `/tmp` вопросы неотличимы от настоящих.
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Сессия-донор: смонтирован один инструмент, корень — временный каталог сценария.
fn session(run_id: &str, root: &Path) -> Session {
    Session {
        id: run_id.into(),
        plan: MountPlan {
            mounted: vec![MOUNTED.into()],
            skipped: Vec::new(),
        },
        caller: PolicyLevel::L5Owner,
        budget_tokens: 43_000,
        token_calibration: None,
        calls: 0,
        subagent_depth: 0,
        root: root.to_path_buf(),
    }
}

/// Запрос-донор: сценарии различаются риском, интерактив ВЫКЛЮЧЕН — иначе `decide` дал бы `Ask`.
fn request(risk: RiskClass) -> CallRequest {
    CallRequest {
        tool: MOUNTED.into(),
        module: "reg".into(),
        risk: ToolRisk::Classified(risk),
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

#[test]
fn an_unanswered_ask_suspends_and_writes_the_question_to_the_inbox() {
    // Дано: R2 просит прав, а спросить некого. Тогда: `Suspended` ПО ВАРИАНТУ и вопрос в
    // inbox — читается ФАЙЛ, а не возвращённое значение: запись и её отсутствие неотличимы.
    let ws = Workspace::new("suspend");
    let mut s = session("run-11c-suspend", &ws.0);
    let fake = Fake::new(Ok("вывод".into()));
    let out = drive(s.handle(&request(RiskClass::R2), &fake)).expect("приостановка — не сбой");
    assert!(matches!(out.verdict, Verdict::Suspended), "спросить некому");
    assert!(fake.seen().is_empty(), "не звали: {:?}", fake.seen());
    let raw = fs::read_to_string(ws.question(&s.id)).expect("вопрос лёг в inbox");
    let back: ApprovalRequest = serde_json::from_str(&raw).expect("вопрос разобран");
    assert_eq!(back.run_id, s.id, "прогон в вопросе");
    assert_eq!(back.args_hash.len(), 64, "sha256: {:?}", back.args_hash);
    assert!(!back.expires_at.is_empty(), "срок ответа задан");
}

#[test]
fn an_expired_question_is_not_a_policy_denial() {
    // Дано: вопрос с ИСТЁКШИМ окном. Тогда: `ApprovalTimeout`, а НЕ `Deny` — «никто не ответил»
    // и «запрещено политикой» разные вещи (В4, закон 6), а `is_err` слил бы их с отказом.
    let ws = Workspace::new("expired");
    let mut s = session("run-11c-expired", &ws.0);
    let file = ws.question(&s.id);
    fs::create_dir_all(file.parent().expect("каталог")).expect("inbox создан");
    let asked = ApprovalRequest {
        run_id: s.id.clone(),
        tool: MOUNTED.into(),
        risk: ToolRisk::Classified(RiskClass::R2),
        args_hash: "a".repeat(64),
        requested_at: "2020-01-01T00:00:00Z".into(),
        expires_at: "2020-01-01T04:00:00Z".into(),
    };
    let wire = serde_json::to_string(&asked).expect("вопрос сериализуем");
    fs::write(&file, wire).expect("вопрос записан");
    let fake = Fake::new(Ok("вывод".into()));
    let req = request(RiskClass::R2);
    assert!(!matches!(decide(&req), Verdict::Deny { .. }), "не отказ");
    let err = drive(s.handle(&req, &fake)).expect_err("окно истекло — сбой сессии");
    assert!(matches!(&err, SessionError::ApprovalTimeout), "{err:?}");
    assert!(err.to_string().contains("approval_timeout"), "{err}");
    assert!(fake.seen().is_empty(), "не звали: {:?}", fake.seen());
}

#[test]
fn r2_and_above_never_create_a_subagent() {
    // Дано: риск R2 и выше. Тогда: глубина субагентов 0, и проверяется ПОЛЕМ, а не текстом
    // инструкции: текстовые запреты модель игнорирует (F1 поймал постфактум-детектор), а
    // правило без наблюдаемого состояния через полгода не отличить (В3).
    for risk in [RiskClass::R2, RiskClass::R3, RiskClass::R4] {
        let ws = Workspace::new(&format!("sub-{risk:?}"));
        let mut s = session(&format!("run-11c-{risk:?}"), &ws.0);
        let fake = Fake::new(Ok("вывод".into()));
        drive(s.handle(&request(risk), &fake)).expect("приостановка — не сбой");
        assert_eq!(s.subagent_depth, 0, "при {risk:?} глубина (В3)");
        assert!(fake.seen().is_empty(), "при {risk:?} не звали");
    }
}

#[test]
fn a_failed_tool_carries_its_receipt_inside_the_error() {
    // Дано: R0 разрешён, инструмент зовут и он падает. Тогда: `SessionError::Tool` с чеком
    // ВНУТРИ ошибки: иначе чек исчез бы, а вызов без следа неотличим от небывшего. По
    // варианту, а не `is_err`: сбой инструмента не отличить от истёкшего окна.
    let ws = Workspace::new("tool");
    let mut s = session("run-11c-tool", &ws.0);
    let fake = Fake::new(Err(ToolError("упал".into())));
    let err = drive(s.handle(&request(RiskClass::R0), &fake)).expect_err("инструмент упал");
    assert_eq!(fake.seen(), [MOUNTED.to_owned()], "шв зван один раз");
    match err {
        SessionError::Tool { source, receipt } => {
            assert_eq!(source, ToolError("упал".into()), "чужой текст цел");
            assert_eq!(receipt.tool, MOUNTED, "чек об этом вызове");
            assert!(!receipt.call_id.is_empty(), "чек без call_id не отыскать");
            assert_eq!(receipt.input_hash.len(), 64, "хеш ушедшего");
        }
        other => panic!("ожидался SessionError::Tool, получено {other:?}"),
    }
}
