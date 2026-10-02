//! Семантика команды `cargo test`: позиционный фильтр против имени target'а.
//!
//! Проверяется запуск настоящего процесса: модуль на Python исполняется ровно так, как его
//! будет исполнять packet-10. Ни одна acceptance-команда при этом НЕ исполняется —
//! инструмент только разбирает строку и печатает вердикт.

use std::path::{Path, PathBuf};
use std::process::Command;

/// `xtask` лежит в корне репозитория, а не в `crates/`, поэтому подниматься на один
/// уровень. Лишний `parent` уводил путь за пределы workspace.
/// `xtask` лежит в корне репозитория, а не в `crates/`, поэтому подниматься на один
/// уровень. Лишний `parent` уводил путь за пределы workspace.
/// `xtask` лежит в корне репозитория, а не в `crates/`, поэтому подниматься на один
/// уровень. Лишний `parent` уводил путь за пределы workspace.
/// `xtask` лежит в корне репозитория, а не в `crates/`, поэтому подниматься на один
/// уровень. Лишний `parent` уводил путь за пределы workspace.
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

fn run(args: &[&str]) -> (String, String, bool) {
    let out = Command::new("python3")
        .arg(script())
        .args(args)
        .output()
        .expect("скрипт должен запускаться");
    (
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
        out.status.success(),
    )
}

/// Каталог фикстуры, свой на каждый тест: имя берётся из имени потока, которое cargo
/// даёт тесту. Общий каталог означал бы, что параллельные тесты затирают фикстуры
/// друг друга прямо в момент прогона.
/// Каталог фикстуры, свой на каждый тест: имя берётся из имени потока, которое cargo
/// даёт тесту. Общий каталог означал бы, что параллельные тесты затирают фикстуры
/// друг друга прямо в момент прогона.
/// Каталог фикстуры, свой на каждый тест: имя берётся из имени потока, которое cargo
/// даёт тесту. Общий каталог означал бы, что параллельные тесты затирают фикстуры
/// друг друга прямо в момент прогона.
fn fixture() -> PathBuf {
    let tag = std::thread::current()
        .name()
        .unwrap_or("unnamed")
        .replace("::", "-");
    let dir = std::env::temp_dir().join(format!("stellwerk-replay-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(".sisyphus/plans")).expect("каталог фикстур");
    dir
}

fn write_packet(dir: &Path, milestone: &str, name: &str, body: &str) {
    let p = dir.join(format!(".sisyphus/plans/{milestone}")).join(name);
    std::fs::create_dir_all(p.parent().unwrap()).expect("milestone dir");
    std::fs::write(&p, body).expect("пакет записан");
}

// INFRA/packet-09 — семантика cargo test: фильтр ≠ target, агрегация
// ═══════════════════════════════════════════════════════════════════════════

/// Прогнать инструмент в режиме `validate` и вернуть stdout: ни одна команда при этом
/// НЕ исполняется, проверяется только разбор и семантика.
/// Прогнать инструмент в режиме `validate` и вернуть stdout: ни одна команда при этом
/// НЕ исполняется, проверяется только разбор и семантика.
/// Прогнать инструмент в режиме `validate` и вернуть stdout: ни одна команда при этом
/// НЕ исполняется, проверяется только разбор и семантика.
fn validate(command: &str) -> String {
    let dir = fixture();
    write_packet(
        &dir,
        "M1",
        "packet-probe.md",
        &format!("# packet-probe\ngoal: GREEN\ntest: {command}\n"),
    );
    let (stdout, stderr, ok) = run(&["validate", "--root", dir.to_str().unwrap()]);
    // stderr в сообщении обязателен: без него диагностика «не работает» выглядит
    // одинаково и при разбираемой ошибке разбора, и при упавшем `cargo metadata`.
    assert!(
        ok,
        "validate должен отрабатывать: stdout={} stderr={}",
        stdout, stderr
    );
    stdout
}

/// Классификация строки: ищем `CLASS=<имя>` в выводе агрегатора.
/// Классификация строки: ищем `CLASS=<имя>` в выводе агрегатора.
#[test]
fn positional_filter_is_not_a_target() {
    let out = validate("cargo test -p stellwerk-registry measure");
    assert!(out.contains("VALID_COMMAND"), "команда валидна: {}", out);
    assert!(
        out.contains("FILTER=measure"),
        "measure обязан быть помечен как FILTER: {}",
        out
    );
    assert!(
        !out.contains("TARGET=measure"),
        "позиционное слово не должно становиться TARGET: {}",
        out
    );
}

/// b. `--test workspace_policy` проверяется по cargo metadata.
/// b. `--test workspace_policy` проверяется по cargo metadata.
#[test]
fn test_target_is_validated_against_metadata() {
    let out = validate("cargo test -p stellwerk-core --test workspace_policy");
    assert!(out.contains("VALID_COMMAND"), "цель существует: {}", out);
    assert!(
        out.contains("TARGET=workspace_policy"),
        "имя цели извлечено из --test: {}",
        out
    );
}

/// c. Несуществующий package → INVALID_PACKAGE.
/// c. Несуществующий package → INVALID_PACKAGE.
#[test]
fn nonexistent_package_is_invalid_package() {
    let out = validate("cargo test -p stellwark-nope --test anything");
    assert!(
        out.contains("INVALID_PACKAGE"),
        "ожидался INVALID_PACKAGE: {}",
        out
    );
}

/// d. Несуществующий target → INVALID_TARGET.
/// d. Несуществующий target → INVALID_TARGET.
#[test]
fn nonexistent_target_is_invalid_target() {
    let out = validate("cargo test -p stellwerk-core --test nope_no_such_target");
    assert!(
        out.contains("INVALID_TARGET"),
        "ожидался INVALID_TARGET: {}",
        out
    );
}

/// e. Несколько строк результата суммируются, а не перезаписывают друг друга.
/// k. Обход песочницы и shell → UNSUPPORTED_COMMAND.
#[test]
fn shell_and_escape_attempts_are_unsupported() {
    for hostile in [
        "cargo test | tee log",
        "cargo test && rm -rf /",
        "cargo test; whoami",
        "bash -c 'cargo test'",
        "sh -c cargo",
        "python3 scripts/plan-lint.py",
        "just check",
        "cargo test --manifest-path ../other/Cargo.toml",
        "cargo test --config net.offline=true",
        "cargo test --target-dir /tmp/elsewhere",
        "cargo test -Z unstable-options",
    ] {
        let out = validate(hostile);
        assert!(
            out.contains("UNSUPPORTED_COMMAND"),
            "должно быть отвергнуто `{hostile}`, получено: {}",
            out
        );
    }
}
