//! Маршрутизация вызова в сессии: что разрешено, что отказано и что попало в чек
//! (M3-11b RED). `handle` — заглушка: четыре сценария падают, прогон компилируется;
//! реализация — M3/packet-12. Вопросы, таймауты и сбои — 11c.
//!
//! Модуль берётся публичным API, а не `#[path]`: тот собрал бы вторую копию и
//! оставил экспорт непроверенным (F-14).

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use stellwerk_core::contract::{DataClass, ModelBinding, PolicyLevel, RetentionClass, RiskClass};
use stellwerk_gateway::gate::{CallRequest, Verdict};
use stellwerk_gateway::mount::MountPlan;
use stellwerk_gateway::permission::ToolRisk;
use stellwerk_gateway::session::{drive, Session, TokenCalibration, ToolBackend, ToolError};

/// Единственный инструмент в плане: всё, чего в нём нет, вне монтажа.
const MOUNTED: &str = "read_file";

/// Подделка шва: считает вызовы и помнит, кому звонили. Счётчик обязателен —
/// «инструмент не звали» недоказуемо, если не видно, что не звали.
struct Fake {
    /// Кому передали вызов, по порядку: пусто — не звали ни разу.
    invoked: Mutex<Vec<String>>,
}

impl Fake {
    /// Подделка без вызовов. Ответ инструмента неважен и всегда одинаков: его
    /// содержимое проверяет 11c вместе со сбоями, здесь важно лишь КТО позван.
    fn new() -> Self {
        Self {
            invoked: Mutex::new(Vec::new()),
        }
    }

    /// Кому звонили, по порядку вызовов.
    fn seen(&self) -> Vec<String> {
        self.invoked.lock().expect("мьютекс цел").clone()
    }
}

impl ToolBackend for Fake {
    /// Записать имя инструмента и отдать заранее известный вывод.
    fn invoke(
        &self,
        tool: &str,
        _input: &str,
    ) -> Pin<Box<dyn Future<Output = Result<String, ToolError>> + Send + '_>> {
        let invoked = &self.invoked;
        let tool = tool.to_owned();
        Box::pin(async move {
            invoked.lock().expect("мьютекс цел").push(tool);
            Ok("вывод".to_owned())
        })
    }
}

/// Сессия-донор: смонтирован один инструмент, риск и полномочия не проверяются.
fn session() -> Session {
    Session {
        id: "run-11b".into(),
        plan: MountPlan {
            mounted: vec![MOUNTED.into()],
            skipped: Vec::new(),
        },
        caller: PolicyLevel::L5Owner,
        budget_tokens: 43_000,
        token_calibration: Some(TokenCalibration {
            provider: "neuraldeep".into(),
            model_id: "m".into(),
            tokens_per_byte: 1.0,
            measured_at: "2026-09-29T00:00:00Z".into(),
            source: "test".into(),
        }),
        calls: 0,
        subagent_depth: 0,
        root: std::env::temp_dir().join("m3-11b"),
    }
}

/// Запрос-донор: интерактив ВКЛЮЧЁН намеренно. Тогда отказ объясним только риском
/// или полномочиями, а не «спросить некого», и `Suspended` не может выдать себя
/// за `Deny` ни в одном сценарии ниже.
fn request() -> CallRequest {
    CallRequest {
        tool: MOUNTED.into(),
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
        interactive: true,
    }
}

#[test]
fn unknown_tool_is_denied_before_the_backend_ever_sees_it() {
    // Дано: инструмента `curl` нет в монтаже, риск R0 и права владельца в порядке
    // — отказать может только монтаж. Тогда: `Deny` с непустой причиной, и шов НЕ
    // зафиксировал ни одного вызова. Список пуст — доказательство; «наверное, не
    // звали» дефектом, при котором инструмент вне плана всё-таки зовётся, не
    // поймало бы ничего.
    let mut s = session();
    let fake = Fake::new();
    let req = CallRequest {
        tool: "curl".into(),
        ..request()
    };
    let out = drive(s.handle(&req, &fake)).expect("отказ — успешный проход, а не сбой");
    let Verdict::Deny { reason } = &out.verdict else {
        panic!("инструмент вне монтажа — отказ, получено {:?}", out.verdict);
    };
    assert!(
        !reason.trim().is_empty(),
        "Deny без причины агенту не сказать, что делать"
    );
    assert!(
        fake.seen().is_empty(),
        "инструмент вне плана зван: {:?}",
        fake.seen()
    );
}

#[test]
fn deny_for_insufficient_level_still_leaves_a_receipt() {
    // Дано: R3 при полномочиях ниже требуемого контрактом. Тогда: `Deny`, но чек
    // обязателен — запрет тоже факт, обязанный попасть в поток, иначе отклонённые
    // попытки не видны в расследовании. И в чеке должен стоять ИМЕННО отказ: тихая
    // попытка и тихий успех выглядели бы в потоке одинаково.
    let mut s = session();
    s.caller = PolicyLevel::L2Contributor;
    let fake = Fake::new();
    let req = CallRequest {
        risk: ToolRisk::Classified(RiskClass::R3),
        // Уровень — в сессии и в запросе: два поля об одном, и расхождение сделало
        // бы сценарий неоднозначным.
        caller: s.caller,
        ..request()
    };
    // Премиса сценария берётся из предиката контракта, а не из выдуманной таблицы:
    // если R3 потребовал бы меньше, отказ объяснялся бы чем-то иным.
    assert!(
        s.caller < RiskClass::R3.required_level(),
        "L2Contributor не ниже требуемого для R3: отказ был бы не про уровень"
    );
    let out = drive(s.handle(&req, &fake)).expect("отказ по правам — успешный проход");
    assert!(
        matches!(&out.verdict, Verdict::Deny { .. }),
        "прав не хватает, а получено {:?}",
        out.verdict
    );
    let Some(receipt) = &out.receipt else {
        panic!("отказ без чека — вызова как будто и не было: {out:?}");
    };
    assert!(
        matches!(&receipt.verdict, Verdict::Deny { .. }),
        "в чеке должен стоять отказ, а не успех: {receipt:?}"
    );
    assert!(
        fake.seen().is_empty(),
        "запрещённое не зовут: {:?}",
        fake.seen()
    );
}

#[test]
fn r5_is_denied_at_once_and_leaves_no_question_behind() {
    // Дано: R5 даже у владельца в интерактивном прогоне. Тогда: `Deny`, а не
    // `Suspended` и не `Ask` — вопрос задавать некому и не о чем, и «запрещено
    // таблицей» не должно выглядеть как «никто не ответил» (В4). Отсечка по риску
    // касается только R5: у R3/R4 вердикт `Ask` разбирается в `decide`, и отсечка
    // здесь сделала бы решения В1/В4 мёртвым кодом.
    let mut s = session();
    let fake = Fake::new();
    let req = CallRequest {
        risk: ToolRisk::Classified(RiskClass::R5),
        ..request()
    };
    let out = drive(s.handle(&req, &fake)).expect("отказ — успешный проход, а не сбой");
    assert!(
        matches!(&out.verdict, Verdict::Deny { .. }),
        "R5 отклоняется сразу, получено {:?}",
        out.verdict
    );
    assert!(
        !matches!(&out.verdict, Verdict::Suspended),
        "«запрещено» и «никто не ответил» — разные состояния (В4)"
    );
    assert!(fake.seen().is_empty(), "R5 не зовут: {:?}", fake.seen());
}

#[test]
fn an_allowed_call_yields_exactly_one_receipt_and_counts_itself() {
    // Дано: смонтированный R0 при правах владельца — разрешён без вопросов. Тогда:
    // ровно ОДИН вызов шва, ровно ОДИН чек и `calls` = 1. Успех без следа
    // неотличим от вызова, которого не было, а невызванный посчитанный вызов —
    // от вызванного.
    let mut s = session();
    let fake = Fake::new();
    let req = request();
    let out = drive(s.handle(&req, &fake)).expect("состоявшийся вызов — успех");
    assert_eq!(out.verdict, Verdict::Allow, "R0 при владельце разрешён");
    assert_eq!(
        fake.seen(),
        [MOUNTED.to_owned()],
        "инструмент позван ровно один раз"
    );
    assert_eq!(s.calls, 1, "невидимый вызов не считается");
    let Some(receipt) = &out.receipt else {
        panic!("вызов без чека не состоялся: {out:?}");
    };
    assert_eq!(
        receipt.verdict,
        Verdict::Allow,
        "в чеке — решение по вызову"
    );
    assert_eq!(receipt.tool, MOUNTED, "чек описывает именно этот вызов");
}

/// Future, готовое только после настоящего пробуждения: на первом опросе оно отдаёт
/// waker второму потоку и возвращает `Pending`, а поток будит его и только затем
/// выставляет признак готовности. Счётчик опросов лежит снаружи, чтобы тест мог
/// отличить ожидание будильника от busy-poll.
struct WakesOnce {
    /// Признак сработавшего будильника; ставится ДО `wake`, иначе разбуженный
    /// исполнитель успел бы опросить future и снова увидеть `Pending`.
    woken: Arc<AtomicBool>,
    /// Сколько раз `drive` опросил future — снаружи, чтобы тест их увидел.
    polls: Arc<AtomicUsize>,
    /// Второй поток уже запущен: на ложном пробуждении спавнить его повторно нельзя.
    spawned: bool,
}

impl Future for WakesOnce {
    type Output = &'static str;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<&'static str> {
        let this = self.get_mut();
        this.polls.fetch_add(1, Ordering::SeqCst);
        if !this.spawned {
            this.spawned = true;
            let waker = cx.waker().clone();
            let woken = Arc::clone(&this.woken);
            std::thread::spawn(move || {
                // Пауза не заменяет пробуждение, а откладывает его заведомо ПОСЛЕ
                // первого `Pending`: иначе `wake` мог бы прийти до того, как
                // исполнитель уснёт, и тест не отличил бы ожидание от busy-poll.
                std::thread::sleep(std::time::Duration::from_millis(50));
                woken.store(true, Ordering::SeqCst);
                waker.wake();
            });
            return Poll::Pending;
        }
        if this.woken.load(Ordering::SeqCst) {
            Poll::Ready("пробуждено")
        } else {
            Poll::Pending
        }
    }
}

#[test]
fn woken_task_continues_after_waker_replacement() {
    // Дано: задача, чей будильник заменён на живой waker `drive`: на первом опросе
    // она отдаёт его второму потоку и возвращает `Pending`, а `Ready` отдаёт только
    // после `wake`. Когда: `drive` опрашивает её. Тогда: задача продолжает исполнение
    // до значения, и опросов мало — исполнитель ждёт будильник, а не крутит
    // `yield_now`, который замаскировал бы потерю пробуждения. Страж нужен потому,
    // что потерянный `wake` при `park` — это не падение, а вечное молчание:
    // `recv_timeout` превращает его в отказ с именем причины, а не в зависшую сьюту.
    let polls = Arc::new(AtomicUsize::new(0));
    let (tx, rx) = std::sync::mpsc::channel();
    let future = WakesOnce {
        woken: Arc::new(AtomicBool::new(false)),
        polls: Arc::clone(&polls),
        spawned: false,
    };
    std::thread::spawn(move || {
        let _ = tx.send(drive(future));
    });
    let out = rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("drive не вернулся за 5 с: пробуждение потеряно — waker не разбудил исполнителя");
    assert_eq!(out, "пробуждено", "drive не довёл future до его значения");
    let seen = polls.load(Ordering::SeqCst);
    assert!(
        seen <= 100,
        "drive опросил future {seen} раз на один будильник: это busy-poll, а не ожидание пробуждения"
    );
}
