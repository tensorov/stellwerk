//! Спецификация M5/packet-23 для `approval.rs`.
//!
//! Файл отвечает на измеренный факт, а не на ожидаемый: перемер файла дал
//! **ноль** таймаутов (77 мутантов = 38 caught + 37 missed + 0 timeout + 2
//! unviable), потому что в модуле нет ни цикла, ни `await`, ни ретрая — зависать
//! в нём нечему. Из 37 пропущенных 30 лежат в блоке дат Хиннанта
//! (`rfc3339_utc`), который вне скоупа пакета, и 7 — в трёх других местах:
//! `risk_label`, окно ответа в `request`, `sha256_hex`, `unix_secs` и guard в
//! `inbox_dir`. Закрыты именно эти семь.
//!
//! Ни один тест здесь не ждёт и не крутит цикл: все функции модуля синхронны.
//! Время проверяется **разбором** выданной строки против отметок, снятых до и
//! после вызова, — а не форматированием тестовым кодом, потому что формат и есть
//! проверяемый код.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use stellwerk_core::contract::RiskClass;
use stellwerk_gateway::permission::ToolRisk;
use stellwerk_task::approval::{inbox_dir, park, request, INBOX_ENV};

/// Секунды epoch до вызова. Внутренний `unix_secs` обязан попасть в интервал
/// между двумя отметками — на этом и держится точность проверки окна.
fn now() -> i64 {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("часы не могут стоять до 1970")
            .as_secs(),
    )
    .expect("epoch помещается в i64")
}

/// `YYYY-MM-DDTHH:MM:SSZ` в секунды epoch. Обратная к проверяемой
/// `civil_from_days` сторона: `days_from_civil` считает дни от года, а она — год
/// от дней, и общая ошибка потребовала бы правки в обоих.
fn epoch_of(s: &str) -> i64 {
    assert_eq!(s.len(), 20, "RFC 3339 UTC — ровно 20 символов, а не `{s}`");
    let (date, time) = s.split_at(10);
    let y: i64 = date[..4].parse().expect("год");
    let m: i64 = date[5..7].parse().expect("месяц");
    let d: i64 = date[8..10].parse().expect("день");
    let hh: i64 = time[1..3].parse().expect("час");
    let mm: i64 = time[4..6].parse().expect("минута");
    let ss: i64 = time[7..9].parse().expect("секунда");
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let days = era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468;
    days * 86_400 + hh * 3_600 + mm * 60 + ss
}

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("stellwerk-packet-23-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch must be creatable");
    dir
}

/// Метка риска на проводе — имя класса, а не `Debug`-форма и не пустая строка.
/// Обе мутации `risk_label -> String` (`String::new()` и `"xyzzy".into()`) падают
/// здесь; `tests/inbox.rs` проверяет только наличие поля `risk`.
#[test]
fn parked_risk_is_the_class_name_on_the_wire() {
    let inbox = scratch("risk");
    for (run, risk, want) in [
        ("run-r0", ToolRisk::Classified(RiskClass::R0), "R0"),
        ("run-r5", ToolRisk::Classified(RiskClass::R5), "R5"),
        ("run-unk", ToolRisk::Unclassified, "unclassified"),
    ] {
        let req = request(run, "t-risk", "shell", risk);
        park(&req, &inbox).expect("park must succeed");
        let raw = std::fs::read_to_string(inbox.join(format!("{run}.json"))).expect("must read");
        let v: serde_json::Value = serde_json::from_str(&raw).expect("must parse");
        assert_eq!(
            v["risk"],
            serde_json::json!(want),
            "метка риска для `{run}`"
        );
    }
}

/// Срок ответа — «сейчас + окно в секундах», а не «сейчас + окно в часах» и не
/// «сейчас». `* -> +` и `* -> /` в `request` и `unix_secs -> -1` дают здесь
/// несовпадение: граница сравнения — отметки до и после вызова, а не «сегодня».
#[test]
fn expires_at_is_now_plus_the_window_in_seconds() {
    let before = now();
    let narrow = request("run-w", "t-w", "shell", ToolRisk::Classified(RiskClass::R4));
    let wide = request("run-w", "t-w", "shell", ToolRisk::Classified(RiskClass::R3));
    let after = now();

    assert_eq!(narrow.window_hours, 4, "R4 — окно 4ч");
    assert_eq!(wide.window_hours, 24, "не R4 — окно суток");
    for (req, secs) in [(&narrow, 14_400_i64), (&wide, 86_400)] {
        let got = epoch_of(&req.expires_at);
        assert!(
            (before + secs..=after + secs).contains(&got),
            "`{got}` — это `{}`, а окно {secs}с от [{before}, {after}] даёт [{}, {}]",
            req.expires_at,
            before + secs,
            after + secs
        );
    }
}

/// Дайджест пустого тела — константа sha2::Sha256 от `b""`, известная независимо
/// от крейта. `tests/inbox.rs` сравнивает длину поля в JSON, а не значение.
#[test]
fn empty_args_digest_is_the_sha256_of_an_empty_body() {
    let req = request("run-d", "t-d", "shell", ToolRisk::Classified(RiskClass::R0));
    assert_eq!(
        req.empty_args_digest, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        "дайджест пустого тела обязан быть sha256 от пустой строки"
    );
}

/// Непустое значение переменной обязано приниматься как есть. Мутация
/// «guard -> true» превращает любой непустой путь в `EmptyInboxEnv`; в
/// `tests/inbox.rs` её не видно, потому что там проверяется только ПУСТОЕ.
/// Переменная трогается в одном тесте намеренно: `set_var` глобален, и два
/// теста в одном бинарнике делили бы его гонкой.
#[test]
fn non_empty_inbox_env_is_taken_verbatim() {
    let previous = std::env::var(INBOX_ENV).ok();
    std::env::set_var(INBOX_ENV, "/var/tmp/stellwerk-packet-23-inbox");
    let outcome = inbox_dir();
    match previous {
        Some(v) => std::env::set_var(INBOX_ENV, v),
        None => std::env::remove_var(INBOX_ENV),
    }
    assert_eq!(
        outcome.expect("непустое значение — не опечатка и обязано приниматься"),
        PathBuf::from("/var/tmp/stellwerk-packet-23-inbox"),
    );
}
