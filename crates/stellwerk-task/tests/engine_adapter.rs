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

use std::process::Command;
use std::time::{Duration, Instant};

use stellwerk_task::m5::{EngineAdapter, EngineError, FixtureEngine, ProbeReport, FIXTURE_ENGINE};

/// Путь, которого нет. Имя выбрано несуществующим, а не «каталогом, где может не
/// оказаться»: тест не должен зависеть от содержимого машины.
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

#[test]
fn a_session_runs_in_the_given_directory_and_passes_arguments_verbatim() {
    let cwd = std::env::temp_dir();
    let fixture = FixtureEngine::new("/bin/true");

    let mut pwd = Command::new("/bin/pwd");
    pwd.current_dir(&cwd);
    let spoke = fixture.session(pwd, Duration::from_secs(5));
    let printed = spoke.expect("сессия").stdout;
    assert_eq!(
        std::fs::canonicalize(printed.trim()).expect("pwd напечатал каталог"),
        std::fs::canonicalize(&cwd).expect("каталог существует"),
        "программа должна была печатать переданный каталог"
    );

    let mut echo = Command::new("/bin/echo");
    echo.args(["alpha beta", "$HOME"]).current_dir(&cwd);
    let spoke = fixture
        .session(echo, Duration::from_secs(5))
        .expect("сессия");
    // `$HOME` осталось строкой: shell, который её развернул бы, не запускался.
    assert_eq!(
        spoke.stdout, "alpha beta $HOME\n",
        "аргументы дословно; stderr: {:?}",
        spoke.stderr
    );
    assert_eq!(spoke.code, Some(0), "код завержения");
}

#[test]
fn a_session_that_outlives_its_budget_is_a_timeout_that_stopped_the_process() {
    let budget = Duration::from_millis(200);
    let started = Instant::now();
    let mut sleep = Command::new("/bin/sleep");
    sleep.arg("30");
    let outcome = FixtureEngine::new("/bin/true").session(sleep, budget);

    match outcome {
        Err(EngineError::TimedOut { program, timeout }) => {
            assert_eq!(program, "/bin/sleep", "виновник назван");
            assert_eq!(timeout, budget, "бюджет назван");
        }
        other => panic!("`sleep 30` при бюджете 200мс — таймаут, а не {other:?}"),
    }
    // Расхождение с бюджетом и есть смысл таймаута: 30с ожидания никто не выдержал бы.
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "ждали {:?} при бюджете 200мс",
        started.elapsed()
    );
}

#[test]
fn a_launch_without_a_program_is_refused_before_anything_is_launched() {
    let outcome =
        FixtureEngine::new("/bin/true").session(Command::new(""), Duration::from_millis(200));

    match outcome {
        Err(EngineError::EmptyArgv) => {}
        other => panic!("запуск без программы — «запускать нечего», а не {other:?}"),
    }
}
