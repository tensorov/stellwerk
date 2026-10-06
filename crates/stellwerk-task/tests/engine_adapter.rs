//! Спецификация M5/packet-02a: контракт запуска сессии и фикстурный движок.
//!
//! Проверяется честность отчёта: путь, которого нет, даёт `available=false` с непустой
//! причиной; заведомо работающая программа — `available=true`; пометка фикстуры видна
//! в отчёте. Фикстура, ведущая себя как настоящий движок, но отчитавшаяся настоящей,
//! дала бы ложный зелёный результат.
//!
//! Настоящий Pi не появляется нигде: доступность читается из метаданных, а сессии идут на
//! программы, которые в системе есть по определению. `sh -c` нет намеренно: shell-строкой
//! нельзя ни доказать отсутствие интерпретации, ни отличить её от самой программы.

use stellwerk_task::m5::{EngineAdapter, EngineError, FixtureEngine, ProbeReport, FIXTURE_ENGINE};

/// Путь, которого нет. Имя выбрано несуществующим, а не «каталогом, где может не/// оказаться»: тест не должен зависеть от содержимого машины.
const MISSING: &str = "/var/tmp/stellwerk-packet-02a/no-such-engine";

#[test]
fn a_program_that_is_not_there_is_unavailable_and_the_reason_names_it() {
    let report = FixtureEngine::new(MISSING)
        .probe()
        .expect("пустой причины не бывает");

    assert!(!report.is_available(), "нет программы — движок недоступен");
    assert_eq!(report.engine(), FIXTURE_ENGINE, "имя движка на отчёте");
    let reason = report.reason();
    assert!(
        reason.contains(MISSING) && reason.len() > MISSING.len(),
        "причина обязана называть путь и говорить сверх него: {reason:?}"
    );
}

#[test]
fn an_executable_program_is_available_and_the_report_calls_itself_a_fixture() {
    let report = FixtureEngine::new("/bin/true")
        .probe()
        .expect("пустой причины не бывает");

    assert!(report.is_available(), "`/bin/true` есть и он исполняем");
    assert!(
        report.engine().contains("fixture"),
        "пометка в имени: {:?}",
        report.engine()
    );
    assert!(
        report.reason().contains("fixture"),
        "пометка в причине: {:?}",
        report.reason()
    );
    assert!(
        report.reason().contains("/bin/true"),
        "причина называет путь: {:?}",
        report.reason()
    );
}

/// Причина обязательна с обеих сторон ответа: «доступен без объяснения» тоже молчит, а
/// молчание при разборе инцидента неотличимо от «никто не ответил» (закон 6).
#[test]
fn a_report_without_a_reason_is_refused_instead_of_claiming_anything_quietly() {
    for built in [
        ProbeReport::available(FIXTURE_ENGINE, "  "),
        ProbeReport::unavailable(FIXTURE_ENGINE, ""),
    ] {
        match built {
            Err(EngineError::NoReason { engine }) => {
                assert_eq!(engine, FIXTURE_ENGINE, "движок назван")
            }
            other => panic!("отчёт без причины — это отказ, а не {other:?}"),
        }
    }
}
