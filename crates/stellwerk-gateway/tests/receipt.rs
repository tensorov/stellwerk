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

// ── M3/packet-15 (F-25) ──
// Пять замков ниже проходят при зелёной сьюитке и всё же различают правило и
// совпадение: каждый называет поломку, которую роняет (F-25).

/// Строка вердикта из потока. Отсутствующий ключ — паника с именем события, а не
/// пустая строка: «ключа нет» и «вердикт пуст» — разные состояния (закон 6), и
/// свёртка первого во второе превратила бы проверку в проверку `"" == ""`.
fn streamed_verdict(event: &Event) -> &str {
    event.payload.get("verdict").map_or_else(
        || panic!("в payload нет ключа verdict: {event:?}"),
        String::as_str,
    )
}

#[test]
fn event_id_is_the_call_id_the_receipt_is_written_under() {
    // Дано: чек вызова с идентификатором, не встречающимся больше нигде.
    // Тогда: `event.id` — ровно этот идентификатор.
    // Поломка, которую ловит: в `to_event` вместо `self.call_id` стоит константа
    // или соседнее поле (`tool`, `module`). Все семь сценариев выше при этом
    // зелёные: они про `payload`, где `call_id` кладётся отдельно. А `append`
    // кладёт `event.id` в первичный ключ (sqlite.rs:331), дубль даёт `Conflict`
    // (sqlite.rs:340), и чек теряется безвозвратно — в потоке второй вызов просто
    // не появляется, а выглядит это как «сеть молчала».
    const CALL_ID: &str = "call-m3-15-77";
    let mut r = receipt(Verdict::Allow);
    r.call_id = CALL_ID.to_owned();
    let event = r.to_event(TASK, WORKSPACE);
    assert_eq!(
        event.id, r.call_id,
        "идентификатор события — это идентификатор вызова: по нему чек лежит \
         в первичном ключе потока, и любая подмена сливает два разных вызова в \
         одну запись, а второй чек теряется без следа"
    );
}

#[test]
fn every_denial_reason_stays_distinguishable_in_the_stream() {
    // Дано: два отказа с РАЗНЫМИ причинами — запрет по классу риска и запрет по
    // правам. Тогда: строки `payload["verdict"]` различаются, и в каждой есть
    // текст именно своей причины.
    // Поломка, которую ловит: свёрнутый энкодер — `format!("{:?}")` заменён на
    // `"allow"/"deny"/"suspended"`. Обе причины дают одну строку `deny` — это
    // ровно та наивная `to_lowercase`, которую коммит `f4195a0` объявляет
    // побеждённой. Формат строки здесь НЕ закреплён: любая кодировка, сохраняющая
    // причину, проходит. Причины взяты без кавычек и переводов строк — `Debug`
    // их экранирует, и тогда проверка подстроки падала бы не из-за потери причины.
    const BY_RISK: &str = "risk_denied: класс R5 запрещён политикой";
    const BY_PERMISSION: &str = "permission_denied: нет права на чтение реестра";
    let by_risk_event = receipt(Verdict::Deny {
        reason: BY_RISK.to_owned(),
    })
    .to_event(TASK, WORKSPACE);
    let by_permission_event = receipt(Verdict::Deny {
        reason: BY_PERMISSION.to_owned(),
    })
    .to_event(TASK, WORKSPACE);
    let by_risk = streamed_verdict(&by_risk_event);
    let by_permission = streamed_verdict(&by_permission_event);
    assert_ne!(
        by_risk, by_permission,
        "две разные причины отказа дали одну строку: расследование перестаёт \
         отличать «запрещено политикой» от «нет прав», и править это нечем"
    );
    assert!(
        by_risk.contains(BY_RISK),
        "текст причины отказа по классу риска не дошёл до потока: {by_risk}"
    );
    assert!(
        by_permission.contains(BY_PERMISSION),
        "текст причины отказа по правам не дошёл до потока: {by_permission}"
    );
}

#[test]
fn payload_carries_the_duration_of_the_call() {
    // Дано: чек с длительностью, отличной от фикстурной. Тогда: в payload есть
    // ключ `duration_ms` и в нём именно эта длительность.
    // Поломка, которую ловит: строка `payload.insert("duration_ms", …)` убрана
    // (ключ выпущен) или наполнена чужим полем. Поле без ключа выпадает из
    // потока, и выпадение неотличимо от «длительность никогда не измерялась»:
    // оба объяснения выглядят одинаково правдоподобно.
    const DURATION_MS: u64 = 4242;
    let mut r = receipt(Verdict::Allow);
    r.duration_ms = DURATION_MS;
    let event = r.to_event(TASK, WORKSPACE);
    assert_eq!(
        event.payload.get("duration_ms").map(String::as_str),
        Some(DURATION_MS.to_string().as_str()),
        "длительность вызова обязана лежать в потоке под своим ключом: без неё \
         время вызова не восстановить, а потерянное поле читается как \
         «никогда не измерялось»"
    );
}

#[test]
fn each_hash_sits_under_its_own_key() {
    // Дано: чек, у которого тела входа и вывода РАЗНЫЕ, а значит и хеши разные.
    // Тогда: под `input_hash` лежит sha256 от INPUT, под `output_hash` — от
    // OUTPUT.
    // Поломка, которую ловит: обмен ключей `input_hash` ↔ `output_hash`. Поиск по
    // значению (сценарий выше) обмен не видит: обе хеш-строки по-прежнему в
    // payload, и все семь прежних сценариев зелёные. А сверка ответа с артефактом
    // идёт по `output_hash` — и после обмена она сравнивает выход с выходом,
    // то есть подтверждает подмену вместо того, чтобы её поймать.
    let input_hash = sha256_hex(INPUT.as_bytes());
    let output_hash = sha256_hex(OUTPUT.as_bytes());
    let event = receipt(Verdict::Allow).to_event(TASK, WORKSPACE);
    assert_eq!(
        event.payload.get("input_hash"),
        Some(&input_hash),
        "под ключом входа лежит хеш входа: иначе запрос сверяется с артефактом \
         не тем телом, и расхождение не видно"
    );
    assert_eq!(
        event.payload.get("output_hash"),
        Some(&output_hash),
        "под ключом выхода лежит хеш вывода: по нему артефакт сверяется с \
         ответом, и подмена делает сверку зелёной на любом выводе"
    );
}

#[test]
fn event_carries_the_timestamp_of_the_call() {
    // Дано: чек со временем вызова, отличным от фикстурного. Тогда: `event.ts` —
    // именно это время.
    // Поломка, которую ловит: `ts: String::new()` (или жёстко заданное время) в
    // `to_event`. Выборка потока упорядочена по `(ts, id)`, поэтому пустой `ts` —
    // не «неаккуратная метка», а сортировка, которая переставляет события
    // относительно реального порядка; по второму компоненту (`id`) порядок уже
    // не восстановить.
    const TS: &str = "2027-01-02T03:04:05Z";
    let mut r = receipt(Verdict::Allow);
    r.ts = TS.to_owned();
    let event = r.to_event(TASK, WORKSPACE);
    assert_eq!(
        event.ts, TS,
        "время вызова обязано дойти до события: по нему поток упорядочен, и \
         пустое время превращает порядок событий в случайный, неразличимый \
         с настоящим"
    );
}
