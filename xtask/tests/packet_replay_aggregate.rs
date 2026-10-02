//! Арифметика прогона: суммирование строк `test result` и итоговая классификация.
//!
//! Проверяется запуск настоящего процесса: модуль на Python исполняется ровно так, как его
//! будет исполнять packet-10. Ни одна acceptance-команда при этом НЕ исполняется —
//! инструмент только разбирает строку и печатает вердикт.

use std::path::PathBuf;
use std::process::Command;

/// `xtask` лежит в корне репозитория, а не в `crates/`, поэтому подниматься на один
/// уровень. Лишний `parent` уводил путь за пределы workspace.
/// Классификация строки: ищем `CLASS=<имя>` в выводе агрегатора.
/// `xtask` лежит в корне репозитория, а не в `crates/`, поэтому подниматься на один
/// уровень. Лишний `parent` уводил путь за пределы workspace.
/// Классификация строки: ищем `CLASS=<имя>` в выводе агрегатора.
/// `xtask` лежит в корне репозитория, а не в `crates/`, поэтому подниматься на один
/// уровень. Лишний `parent` уводил путь за пределы workspace.
/// `xtask` лежит в корне репозитория, а не в `crates/`, поэтому подниматься на один
/// уровень. Лишний `parent` уводил путь за пределы workspace.
fn script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("корень workspace")
        .join("scripts/packet_replay.py")
}

fn agg(input: &str, exit_code: i32) -> String {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new("python3")
        .arg(script())
        .args(["aggregate", "--exit-code", &exit_code.to_string()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("aggregator must spawn");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(input.as_bytes())
        .expect("write fixture");
    let out = child.wait_with_output().expect("aggregator must finish");
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn class_of(stdout: &str) -> String {
    stdout
        .lines()
        .find_map(|l| l.strip_prefix("CLASS="))
        .unwrap_or("<нет CLASS>")
        .trim()
        .to_owned()
}

fn field_of(stdout: &str, key: &str) -> String {
    stdout
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{key}=")))
        .unwrap_or("<нет>")
        .trim()
        .to_owned()
}

/// a. Позиционный фильтр `measure` не становится target'ом.
/// Одна строка FAILED среди нескольких ok → FAIL.
///
/// Ветка `failed > 0` не была покрыта ни одним тестом: зелёные строки суммировались
/// корректно, но падение где-то среди них молча проходило как PASS.
#[test]
fn one_failed_line_among_ok_is_fail() {
    let fixture = "\
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out";
    let out = agg(fixture, 0);
    assert_eq!(field_of(&out, "FAILED"), "1", "{}", out);
    assert_eq!(
        class_of(&out),
        "FAIL",
        "одна упавшая строка обесценивает прогон целиком: {}",
        out
    );
}

#[test]
fn several_result_lines_are_summed() {
    let fixture = "\
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out";
    let out = agg(fixture, 0);
    assert_eq!(field_of(&out, "RESULT_LINES"), "3", "три строки: {}", out);
    assert_eq!(field_of(&out, "PASSED"), "22", "5+7+10=22: {}", out);
    assert_eq!(field_of(&out, "FILTERED"), "3", "0+3+0: {}", out);
}

/// f. Первая строка 0/49, вторая 22 → PASS с passed_total=22.
///    Это ровно тот случай, где взятие первой строки дало бы ложный ноль.
/// f. Первая строка 0/49, вторая 22 → PASS с passed_total=22.
///    Это ровно тот случай, где взятие первой строки дало бы ложный ноль.
#[test]
fn zero_then_passed_sums_to_pass() {
    let fixture = "\
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 49 filtered out
test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out";
    let out = agg(fixture, 0);
    assert_eq!(
        field_of(&out, "PASSED"),
        "22",
        "взята сумма, не первая: {}",
        out
    );
    assert_eq!(
        class_of(&out),
        "PASS",
        "filtered не делает результат слабым: {}",
        out
    );
}

/// g. Только 0 passed / 49 filtered → INCONCLUSIVE_ZERO_EXECUTED.
/// g. Только 0 passed / 49 filtered → INCONCLUSIVE_ZERO_EXECUTED.
#[test]
fn zero_passed_with_filtered_is_inconclusive() {
    let fixture = "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 49 filtered out";
    let out = agg(fixture, 0);
    assert_eq!(class_of(&out), "INCONCLUSIVE_ZERO_EXECUTED", "{}", out);
    assert_eq!(field_of(&out, "PASSED"), "0", "{}", out);
}

/// h. Exit 0 без строк результата → INCONCLUSIVE_NO_RESULT.
/// h. Exit 0 без строк результата → INCONCLUSIVE_NO_RESULT.
#[test]
fn exit_zero_without_result_lines_is_no_result() {
    let out = agg("Compiling stellwerk-core\nFinished\n", 0);
    assert_eq!(class_of(&out), "INCONCLUSIVE_NO_RESULT", "{}", out);
    assert_eq!(field_of(&out, "RESULT_LINES"), "0", "{}", out);
}

/// i. Exit 101 без строк результата → FAIL.
/// i. Exit 101 без строк результата → FAIL.
#[test]
fn nonzero_exit_without_result_lines_is_fail() {
    let out = agg("error[E0308]: mismatched types\n", 101);
    assert_eq!(class_of(&out), "FAIL", "ненулевой код решает: {}", out);
}

/// j. Timeout → FAIL.
/// j. Timeout → FAIL.
#[test]
fn timeout_is_fail() {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new("python3")
        .arg(script())
        .args(["aggregate", "--exit-code", "0", "--timed-out"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n")
        .expect("write");
    let out = child.wait_with_output().expect("wait");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert_eq!(
        class_of(&stdout),
        "FAIL",
        "timeout сильнее любого числа тестов: {}",
        stdout
    );
}

/// k. Обход песочницы и shell → UNSUPPORTED_COMMAND.
/// l. Discovery без распознаваемого списка → selected_by_list=null, а не ноль.
#[test]
fn unusable_discovery_is_null_not_zero() {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new("python3")
        .arg(script())
        .args(["aggregate", "--exit-code", "0"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"running 0 tests\n")
        .expect("write");
    let out = child.wait_with_output().expect("wait");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        stdout.contains("SELECTED_BY_LIST=null"),
        "недостоверный discovery обязан быть null, а не нулём: {}",
        stdout
    );
    assert!(
        !stdout.contains("CLASS=INCONCLUSIVE_ZERO_EXECUTED"),
        "ложный INCONCLUSIVE запрещён: {}",
        stdout
    );
}
