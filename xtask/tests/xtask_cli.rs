//! Интеграционные тесты CLI xtask: проверяется код возврата, а не текст.
//!
//! Зачем отдельный файл, а не тест в `main.rs`: `main.rs` — доверенная база, и
//! предикат, живущий в ней же, не может проверять саму обёртку.
//!
//! Коды утверждаются как «ненулевой», а не конкретное число: `cargo test`
//! возвращает 101, 1 или 4 в зависимости от того, чем именно сломалось. Поэтому
//! отказ опознаётся по маркеру из stderr, взятому дословно из ветки отказа
//! `main.rs`, и не зависит от того, какое число `cargo` выбрал на этой неделе.

use std::path::PathBuf;
use std::process::{Command, Output};

/// Строка таблицы, на которой держится весь позитивный смысл сьютки.
///
/// Изначально это была `red-gate-session-contract`. M3-12a позеленел её, и
/// обёртка отвергла её с «тест зелёный» — то есть повела себя ровно как задумано,
/// а падение сьютки выглядело как баг в обёртке (F-26). Сейчас единственный
/// красный бинарник — `session_failure` (M3-11c): 4 падения на двух явных
/// заглушках `unimplemented!("M3/packet-12b: ...")`.
const POSITIVE_FIXTURE: &str = "red-gate-session-failure";

/// Маркеры отказов — дословно из `main.rs`, по одному на каждую причину.
const GREEN_IS_NOT_RED: &str = "тест зелёный";
const NOT_BUILT: &str = "не собрался";
const CRASHED: &str = "не удалось выполнить";

/// Рабочий каталог дочернего `cargo` — корень workspace, как при настоящем
/// `cargo xtask` из алиаса. Иначе разрешение `-p` зависело бы от того, откуда
/// запущен тест.
fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("у крейта xtask есть родитель — корень workspace")
        .to_path_buf()
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(args)
        .current_dir(workspace_root())
        .output()
        .unwrap_or_else(|err| panic!("не удалось запустить xtask {args:?}: {err}"))
}

/// Код возврата. `Some(0)` — успех, `Some(н)` — отказ; отсутствие кода означало
/// бы, что процесс убит сигналом, и само по себе это не результат гейта.
fn code(args: &[&str]) -> i32 {
    run(args)
        .status
        .code()
        .expect("xtask не должен завершаться сигналом")
}

fn assert_not_unknown(out: &Output, task: &str) {
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("неизвестная задача"),
        "строка {task:?} не найдена в таблице: {stderr}"
    );
}

fn assert_reason(out: &Output, marker: &str) {
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains(marker), "отказ не назван: {stderr}");
}

/// Предварительное условие для каждого позитивного утверждения сьютки: строка
/// `POSITIVE_FIXTURE` обязана быть красной ПРЯМО СЕЙЧАС, иначе гейт нечего
/// подтверждать.
///
/// Вызывается и из одноимённого теста — чтобы страховка была видна по имени в
/// выводе, — и из каждого позитивного теста, поэтому пройти вхолостую нельзя
/// независимо от того, в каком порядке и в каких потоках harness запустит тесты.
/// Вакантный успех здесь запрещён: он выглядит как уверенное доказательство и
/// не доказывает ничего.
fn assert_positive_fixture_is_red() {
    let out = run(&[POSITIVE_FIXTURE]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    match out.status.code() {
        Some(0) => {}
        Some(_) if stderr.contains(GREEN_IS_NOT_RED) => panic!(
            "положительная фикстура {POSITIVE_FIXTURE:?} больше не красная — пакет 12b \
             позеленел её; переприцели эту сьютку на текущий КРАСНЫЙ бинарник. Ослаблять \
             утверждение нельзя: обёртка обязана отказывать зелёному бинарню, это и есть её \
             работа. Причина отказа: {stderr}"
        ),
        Some(_) if stderr.contains(NOT_BUILT) => panic!(
            "бинарник за строкой {POSITIVE_FIXTURE:?} не собирается — RED-фикстуры больше \
             нет. Переприцели сьютку; молча проходить здесь нельзя. Вывод: {stderr}"
        ),
        Some(_) if stderr.contains(CRASHED) => {
            panic!("дочерний cargo за {POSITIVE_FIXTURE:?} не выполнился: {stderr}")
        }
        other => panic!(
            "непредвиденный код {other:?} у положительной фикстуры {POSITIVE_FIXTURE:?}. \
             Если строка выпала из таблицы, позовится и ветка «неизвестная задача», и сьютка \
             развалится вхолостую. Вывод: {stderr}"
        ),
    }
}

#[test]
fn unknown_task_exits_2() {
    for name in ["rm-rf-slash", "red-gate-", "red-gate-session-contract2"] {
        assert_eq!(
            code(&[name]),
            2,
            "неизвестная задача {name:?} обязана давать 2"
        );
    }
}

#[test]
fn no_arguments_exits_2() {
    let out = run(&[]);
    assert_eq!(out.status.code(), Some(2));
    // Подсказка обязана быть собрана из обеих таблиц, иначе red-gate не discoverable.
    let stderr = String::from_utf8_lossy(&out.stderr);
    for expected in [
        "worker-tests",
        "gate",
        "red-gate-session-contract",
        "red-gate-session-failure",
        "red-gate-permission",
    ] {
        assert!(
            stderr.contains(expected),
            "подсказка молчит про {expected:?}: {stderr}"
        );
    }
}

/// Строка из подсказки обязана быть ровно именем строки таблицы: обрезка или
/// подстановка соседнего имени — тот же класс дефекта, что и «похожие имена».
#[test]
fn lookup_does_not_accept_neighbouring_names() {
    for name in [
        "RED-GATE-SESSION",
        "red-gate-session ",
        "red-gate-session-failure2",
    ] {
        assert_eq!(
            code(&[name]),
            2,
            "{name:?} не должен подходить к строке таблицы"
        );
    }
}

/// Страховка от тихой порчи позитивной фикстуры.
///
/// Зачем она нужна. Имя RED-бинаря, зашитое в тест, гниёт в ту же минуту, когда
/// пакет становится GREEN, — а в этом репозитории это нормальное направление
/// движения, потому что это и есть цикл TDD: RED написан, чтобы позеленеть.
/// Без страховки следующий человек видит упавшую сьютку, делает вывод, что
/// сломалась обёртка, и смягчает ровно ту проверку, которая его поймала.
///
/// Так было и здесь: `red-gate-session-contract` был красным на момент написания,
/// M3-12a его позеленел, обёртка честно отказала, и падение выглядело как баг в
/// ней (F-26). Поэтому страховка обязана срабатывать ДО того, как позитивный
/// тест опрётся на фикстуру, и требовать переприцеливания, а не проходить
/// вхолостую.
#[test]
fn positive_fixture_session_failure_is_still_red() {
    assert_positive_fixture_is_red();
}

#[test]
fn red_gate_over_red_binary_exits_zero() {
    assert_positive_fixture_is_red();
    assert_eq!(code(&[POSITIVE_FIXTURE]), 0);
}

#[test]
fn red_gate_rejects_green_binary() {
    // `permission` зелёный. Обёртка, подтверждающая падение там, где тест
    // проходит, опаснее отсутствия обёртки: она превращает зелёный набор в
    // «доказательство» RED-пакета. Поэтому зелёный обязан давать отказ.
    //
    // Проверяется и причина отказа, а не только его наличие: «строка не найдена»
    // тоже даёт ненулевой код, и без этого утверждения тест удовлетворялся бы
    // заглушкой, в которой таблицы нет вовсе.
    let out = run(&["red-gate-permission"]);
    assert_ne!(
        out.status.code(),
        Some(0),
        "зелёный бинарник не должен проходить RED-гейт"
    );
    assert_not_unknown(&out, "red-gate-permission");
    assert_reason(&out, GREEN_IS_NOT_RED);
}

/// Строки, которые были RED, а стали GREEN, обязаны и дальше отказывать — и
/// обязаны делать это ИМЕННО как зелёному, а не как «строка не найдена».
///
/// Это не мёртвый груз: пока строки живы в таблице, они фиксируют, что обёртка
/// отличает «позеленело» от «сломано». Снимок на 2026-09-29: обе дают 4 с
/// маркером «тест зелёный». Если обёртку когда-нибудь ослабят до штампа успеха,
/// тест упадёт здесь, а не на чужом пакете.
#[test]
fn rows_over_now_green_binaries_refuse() {
    for row in ["red-gate-session-contract", "red-gate-session"] {
        let out = run(&[row]);
        assert_ne!(
            out.status.code(),
            Some(0),
            "{row:?} позеленел и обязан отказывать, а не сертифицировать успех"
        );
        assert_not_unknown(&out, row);
        assert_reason(&out, GREEN_IS_NOT_RED);
    }
}

/// Инвариант из шапки `main.rs`: из `argv` в дочерний процесс не уходит ничего.
/// Если бы лишний флаг перенаправлялся, cargo упал бы на незнакомом аргументе
/// ещё на шаге `--no-run` и код был бы ненулевым — вместо ожидаемого нуля.
///
/// Фикстура обязана быть красной: пересылка аргумента в строку, которая и так
/// возвращает отказ, не доказывает ничего о пересылке.
#[test]
fn extra_argument_is_not_forwarded_to_the_child() {
    assert_positive_fixture_is_red();
    assert_eq!(code(&[POSITIVE_FIXTURE, "--exec=/bin/false"]), 0);
}

/// Аттестуется код возврата, а не текст: если бы «подтверждением» падения мог
/// стать stdout, текст пришлось бы парсить — и ошибка парсинга стала бы
/// успехом. Строка таблицы при этом обязана быть найдена, иначе пустой stdout
/// получился бы не потому, что гейт молчит, а потому что он не запускался.
///
/// Охвачены все три исхода: сертификат падения, отказ зелёному, отказ позеленевшей строке.
#[test]
fn red_gate_writes_nothing_to_stdout() {
    for args in [
        [POSITIVE_FIXTURE].as_slice(),
        ["red-gate-permission"].as_slice(),
        ["red-gate-session-contract"].as_slice(),
    ] {
        let out = run(args);
        assert!(
            out.stdout.is_empty(),
            "{args:?} писал в stdout: {}",
            String::from_utf8_lossy(&out.stdout)
        );
        assert_not_unknown(&out, args[0]);
    }
}
