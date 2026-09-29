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
    drive, ApprovalRequest, Session, SessionError, TokenCalibration, ToolBackend, ToolError,
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
    assert_eq!(
        back.empty_args_digest.len(),
        64,
        "sha256: {:?}",
        back.empty_args_digest
    );
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
        empty_args_digest: "a".repeat(64),
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
            assert_eq!(
                receipt.empty_input_digest.len(),
                64,
                "дайджест пустого тела входа, а не пустая строка"
            );
        }
        other => panic!("ожидался SessionError::Tool, получено {other:?}"),
    }
}

/// Калибровка-донор: пара «провайдер/модель» как у `request`, величина произвольная.
/// Замер Qwen и схем четырёх инструментов PolyForge не константа системы, и подставить его
/// здесь значило бы объявить ценой байта измеренное для одной модели; единица «токен на
/// байт» цену завышает, то есть усекает раньше любой реальной калибровки.
fn calibration(tokens_per_byte: f64) -> TokenCalibration {
    TokenCalibration {
        provider: "neuraldeep".into(),
        model_id: "m".into(),
        tokens_per_byte,
        measured_at: "2026-09-29T00:00:00Z".into(),
        source: "test".into(),
    }
}

/// Отказ обязан называть причину и не выдавать себя за два уже объявленных: «инструмент
/// упал» и «никто не ответил на вопрос» — про другое (В4, закон 6). Имя будущего варианта
/// здесь НЕ названо: его ещё нет, а упоминание несуществующего варианта не дало бы тесту
/// собраться — RED обязан компилироваться, иначе он ничего не говорит об усечении.
fn assert_calibration_refusal(err: &SessionError) {
    assert!(
        !matches!(err, SessionError::Tool { .. }),
        "инструмент ответил: {err:?}"
    );
    assert!(
        !matches!(err, SessionError::ApprovalTimeout),
        "окно ответа тут ни при чём: {err:?}"
    );
    let named = err.to_string().to_lowercase();
    assert!(
        named.contains("calibrat") || named.contains("tokens_per_byte"),
        "причина обязана называться, а не молчать: {named}"
    );
}

#[test]
fn a_calibrated_oversized_output_is_truncated_and_the_full_body_lands_in_an_artifact() {
    // Дано: калибровка СВОЕЙ модели и вывод заведомо выше порога. Тогда: `truncated` и в
    // чеке, и в shaping, полное тело — в артефакте, а хеш считается по полному телу:
    // хеш по усечённому тексту сорвал бы сверку артефакта с его же хешем (REVIEW §3.3).
    let ws = Workspace::new("truncated");
    let mut s = session("run-12c-truncated", &ws.0);
    s.token_calibration = Some(calibration(1.0));
    let body = "вывод".repeat(40_000);
    let fake = Fake::new(Ok(body.clone()));
    let out = drive(s.handle(&request(RiskClass::R0), &fake)).expect("вызов состоялся");
    let shaped = out.shaped.expect("усечение по бюджету решается в 12c");
    let receipt = out.receipt.expect("чек у состоявшегося вызова");
    assert!(shaped.truncated, "вывод за порогом, а поместился");
    assert!(
        receipt.truncated,
        "чек обязан сказать об усечении: иначе вывод ушёл молча"
    );
    assert_eq!(
        shaped.sha256.len(),
        64,
        "хеш определён и для усечённого: {shaped:?}"
    );
    let artifact = shaped
        .artifact_path
        .expect("усечённый вывод без артефакта — молчание");
    assert_eq!(
        fs::read_to_string(&artifact).expect("артефакт прочитан"),
        body,
        "в артефакте полное тело, а не усечённый текст"
    );
    assert!(
        shaped.text.starts_with("вывод"),
        "в контекст ушло начало вывода"
    );
    assert!(
        shaped.text.len() < body.len(),
        "в контексте тело короче полного"
    );
    // Артефакт назван этим хешем — сверка идёт глазами, и только полное тело даёт такой
    // хеш; усечённый текст дал бы другой, и расхождение обнаружилось бы при сверке.
    let named = format!("{}.txt", shaped.sha256);
    assert_eq!(
        artifact.file_name().expect("имя артефакта"),
        std::ffi::OsStr::new(&named),
        "имя артефакта — хеш полного тела"
    );
    assert_eq!(
        receipt.full_output_hash, shaped.sha256,
        "один хеш о полном теле"
    );
}

#[test]
fn an_output_that_fits_carries_no_artifact_path() {
    // Дано: калибровка есть, а тело в порог поместилось. Тогда: `truncated = false` и
    // `artifact_path = None` — путь на несуществующий файл вскрылся бы через месяц, и
    // никак не раньше.
    let ws = Workspace::new("fits");
    let mut s = session("run-12c-fits", &ws.0);
    s.token_calibration = Some(calibration(1.0));
    let fake = Fake::new(Ok("вывод".into()));
    let out = drive(s.handle(&request(RiskClass::R0), &fake)).expect("вызов состоялся");
    let shaped = out
        .shaped
        .expect("усечение решается всегда, даже когда тело поместилось");
    assert!(!shaped.truncated, "короткий вывод помещается в порог");
    assert!(
        shaped.artifact_path.is_none(),
        "путь на несуществующий файл — тихий сбой: {:?}",
        shaped.artifact_path
    );
    assert_eq!(shaped.text, "вывод", "тело уходит в контекст без изъятий");
}

#[test]
fn a_session_without_a_calibration_refuses_instead_of_pretending_output_is_free() {
    // Дано: `token_calibration: None` — замера нет. Тогда: явный отказ, а не «вывод
    // ничего не стоит»: `unwrap_or(0.0)` дал бы «влезает» и молчаливое отсутствие
    // усечения, то есть неизвестное, превращённое в разрешение (закон 6).
    let ws = Workspace::new("uncalibrated");
    let mut s = session("run-12c-uncalibrated", &ws.0);
    assert!(
        s.token_calibration.is_none(),
        "донор без калибровки — вход сценария"
    );
    let fake = Fake::new(Ok("вывод".repeat(40_000)));
    let err = drive(s.handle(&request(RiskClass::R0), &fake))
        .expect_err("нет замера — не значит «вывод бесплатен»");
    assert_calibration_refusal(&err);
}

#[test]
fn a_calibration_measured_on_another_model_is_refused_not_applied() {
    // Дано: та же величина, но замер чужой пары «провайдер/модель». Тогда: отказ, а не
    // применение: усечение по чужому токенизатору выглядит правдоподобно и неверно, и
    // отличить его от своего по результату нечем.
    let ws = Workspace::new("foreign");
    let mut s = session("run-12c-foreign", &ws.0);
    let mut foreign = calibration(1.0);
    foreign.provider = "anthropic".into();
    foreign.model_id = "claude".into();
    s.token_calibration = Some(foreign);
    let fake = Fake::new(Ok("вывод".repeat(40_000)));
    let err = drive(s.handle(&request(RiskClass::R0), &fake))
        .expect_err("замер чужой модели — не наш замер");
    assert_calibration_refusal(&err);
}

#[test]
fn an_unusable_tokens_per_byte_is_refused_instead_of_becoming_a_fitting_budget() {
    // Дано: ноль, отрицательное, бесконечное или `NaN`. Тогда: отказ. Ноль — это «вывод
    // ничего не стоит», бесконечное — «обрезать всё»: и то и другое выглядит решением с
    // выбором порога, которого никто не принимал, а `shape()` различает «не поместилось»
    // и «нечем мерить».
    for bad in [0.0, -1.0, f64::INFINITY, f64::NAN] {
        let ws = Workspace::new(&format!("bad-{bad:?}"));
        let mut s = session(&format!("run-12c-bad-{bad:?}"), &ws.0);
        s.token_calibration = Some(calibration(bad));
        let fake = Fake::new(Ok("вывод".repeat(40_000)));
        let err = drive(s.handle(&request(RiskClass::R0), &fake))
            .expect_err("при {bad:?} усечения не бывает — только отказ");
        assert_calibration_refusal(&err);
    }
}
