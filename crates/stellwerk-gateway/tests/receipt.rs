//! Тесты чека вызова: запись КАЖДОГО вызова в поток, вызов без чека не состоялся
//! (M3-09 RED). RED: `to_event` — заглушка, шесть сценариев падают, прогон
//! компилируется. Модуль берётся публичным API, а не `#[path]`: тот скомпилировал
//! бы вторую копию и оставил экспорт непроверенным (F-14).

use sha2::{Digest, Sha256};
use stellwerk_core::contract::{Event, EventSource, RiskClass};
use stellwerk_gateway::receipt::{Receipt, Verdict};

/// Задача, в чей поток пишется чек: `append` кладёт `payload["task_id"]` в
/// отдельную колонку, и `list_by_task` выбирает по ней.
const TASK: &str = "task-m3-09";
/// Тела вызовов: заметно длиннее хеша, чтобы утечка тела не спуталась с хешем.
const INPUT: &str = r#"{"path":"crates/stellwerk-gateway/src/receipt.rs"}"#;
const OUTPUT: &str = "M3-09: поверхность чека объявлена, тело — M3-10";
/// Рабочее место вызова в сценариях, которым оно безразлично. Передаётся явно:
/// значение выдаёт вызывающая сторона, и подмена его константой осталась бы
/// незаметной, пока рабочих мест больше одного.
const WORKSPACE: &str = "ws-stellwerk";

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Чек вызова с заданным вердиктом. Хеши полей считает тест, а не модуль: сверка
/// с эталоном бессмысленна, когда эталон и сравниваемое считает один и тот же код.
fn receipt(verdict: Verdict) -> Receipt {
    Receipt {
        call_id: "call-1".into(),
        tool: "read_file".into(),
        module: "reg".into(),
        verdict,
        risk: RiskClass::R2,
        input_hash: sha256_hex(INPUT.as_bytes()),
        output_hash: sha256_hex(OUTPUT.as_bytes()),
        truncated: false,
        duration_ms: 12,
        ts: "2026-09-28T10:00:00Z".into(),
    }
}

#[test]
fn every_call_becomes_a_gateway_call_event() {
    // Дано: чек разрешённого вызова R2 из модуля reg. Тогда: событие потока —
    // `gateway.call` от System, актёр — модуль, риск из чека, workspace заполнен.
    let event = receipt(Verdict::Allow).to_event(TASK, WORKSPACE);
    assert_eq!(event.event_type, "gateway.call");
    assert_eq!(event.source, EventSource::System);
    assert_eq!(event.actor, "reg", "актёр — модуль, а не агент");
    assert_eq!(event.risk, RiskClass::R2, "риск берётся из чека");
    assert!(event.workspace.is_some(), "workspace обязан быть заполнен");
}

#[test]
fn payload_carries_the_keys_that_make_a_call_findable() {
    // Дано: чек вызова. Тогда: payload несёт пять обязательных ключей, а `task_id`
    // особенно — без него `list_by_task` вызов не найдёт, и вызов окажется
    // несостоявшимся при полностью «успешном» оформлении чека.
    let event = receipt(Verdict::Allow).to_event(TASK, WORKSPACE);
    let payload = &event.payload;
    assert_eq!(payload.get("task_id").map(String::as_str), Some(TASK));
    assert_eq!(payload.get("call_id").map(String::as_str), Some("call-1"));
    assert_eq!(payload.get("tool").map(String::as_str), Some("read_file"));
    assert_eq!(payload.get("truncated").map(String::as_str), Some("false"));
    assert!(payload.contains_key("verdict"), "вердикт в payload");
}

#[test]
fn denied_call_is_recorded_too() {
    // Дано: запрещённый вызов с причиной, плюс разрешение и «никто не ответил».
    // Тогда: запрет тоже даёт событие, и в потоке он отличим и от разрешения,
    // и от `Suspended` — иначе расследование не отличит отказ от молчания.
    let reason = "risk_denied: класс R5 запрещён".to_string();
    let denied = receipt(Verdict::Deny { reason }).to_event(TASK, WORKSPACE);
    assert_eq!(denied.event_type, "gateway.call", "запрет — тоже вызов");
    assert_eq!(denied.risk, RiskClass::R2, "риск чек сохраняет");
    let verdict_of = |event: &Event| event.payload.get("verdict").cloned().unwrap_or_default();
    let allow = receipt(Verdict::Allow).to_event(TASK, WORKSPACE);
    let suspended = receipt(Verdict::Suspended).to_event(TASK, WORKSPACE);
    assert_ne!(verdict_of(&denied), verdict_of(&allow), "запрет ≠ allow");
    let denied_verdict = verdict_of(&denied);
    assert_ne!(
        denied_verdict,
        verdict_of(&suspended),
        "запрет ≠ Suspended (В4)"
    );
}

#[test]
fn receipt_hashes_match_sha256_of_the_bodies() {
    // Дано: чек, хеши полей которого посчитаны тестом от тел. Тогда: в payload
    // лежат sha256 от тех же тел. Ищем по значению, а не по имени ключа: пакет
    // называет пять ключей payload и не называет хеши, а не хешем в потоке быть
    // не может — иначе запись вызова непроверяема.
    let event = receipt(Verdict::Allow).to_event(TASK, WORKSPACE);
    let carries = |want: &str| event.payload.values().any(|value| value == want);
    assert!(carries(&sha256_hex(INPUT.as_bytes())), "хеша входа нет");
    assert!(carries(&sha256_hex(OUTPUT.as_bytes())), "хеша выхода нет");
}

#[test]
fn bodies_never_land_in_the_receipt() {
    // Дано: чек с телами входа и вывода. Тогда: ни одно значение payload не
    // содержит тела: секрет из аргументов не уезжает в поток (REVIEW §3.2).
    let event = receipt(Verdict::Allow).to_event(TASK, WORKSPACE);
    let leaks = |body: &str| event.payload.values().any(|value| value.contains(body));
    assert!(!leaks(INPUT) && !leaks(OUTPUT), "тело в чеке: {event:?}");
}

#[test]
fn empty_output_still_hashes() {
    // Дано: вызов с пустым выводом. Тогда: чек несёт канонический sha256 пустого
    // тела, а не пустую строку: `""` означала бы «не посчитали» и была бы
    // неотличима от значения, то есть подделкой под присутствие данных.
    const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    let mut r = receipt(Verdict::Allow);
    r.output_hash = sha256_hex(b"");
    let event = r.to_event(TASK, WORKSPACE);
    assert!(
        event.payload.values().any(|value| value == EMPTY_SHA256),
        "хеша пустого вывода нет: {event:?}"
    );
}

#[test]
fn receipt_carries_the_workspace_of_the_calling_side() {
    // Дано: вызов из другого рабочего места федерации. Тогда: чек несёт именно
    // переданное значение, а не константу модуля: подстановка выглядит полным
    // чеком, указывает не туда, где был вызов, и при единственном рабочем месте
    // отличить её от значения нечем — второе место и есть тот случай, где отличает.
    const ELSEWHERE: &str = "ws-второе";
    let event = receipt(Verdict::Allow).to_event(TASK, ELSEWHERE);
    assert_eq!(
        event.workspace.as_deref(),
        Some(ELSEWHERE),
        "чек несёт рабочее место вызова, а не подстановку: {event:?}"
    );
}
