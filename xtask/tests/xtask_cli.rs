//! Интеграционные тесты CLI xtask: проверяется код возврата, а не текст.
//!
//! Зачем отдельный файл, а не тест в `main.rs`: `main.rs` — доверенная база, и
//! предикат, живущий в ней же, не может проверять саму обёртку.
//!
//! Коды утверждаются как «ненулевой», а не конкретное число: `cargo test`
//! возвращает 101, 1 или 4 в зависимости от того, чем именно сломалось, и
//! утверждение о конкретном числе роняло бы гейт из-за любой не связанной с ним
//! поломки.

use std::path::PathBuf;
use std::process::{Command, Output};

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

const GREEN_IS_NOT_RED: &str = "тест зелёный";

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

#[test]
fn red_gate_over_red_binary_exits_zero() {
    // session_contract и session честно красные: заглушка `handle` не реализована.
    // Когда M3-12 её реализует, эти строки таблицы снимаются — вместе с тестом,
    // а не ослаблением утверждения.
    assert_eq!(code(&["red-gate-session-contract"]), 0);
    assert_eq!(code(&["red-gate-session"]), 0);
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

/// Инвариант из шапки `main.rs`: из `argv` в дочерний процесс не уходит ничего.
/// Если бы лишний флаг перенаправлялся, cargo упал бы на незнакомом аргументе и
/// код был бы ненулевым — вместо ожидаемого нуля.
#[test]
fn extra_argument_is_not_forwarded_to_the_child() {
    assert_eq!(code(&["red-gate-session-contract", "--exec=/bin/false"]), 0);
}

/// Аттестуется код возврата, а не текст: если бы «подтверждением» падения мог
/// стать stdout, текст пришлось бы парсить — и ошибка парсинга стала бы
/// успехом. Строка таблицы при этом обязана быть найдена, иначе пустой stdout
/// получился бы не потому, что гейт молчит, а потому что он не запускался.
#[test]
fn red_gate_writes_nothing_to_stdout() {
    for args in [
        ["red-gate-session-contract"].as_slice(),
        ["red-gate-permission"].as_slice(),
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
