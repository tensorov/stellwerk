//! Песочница: команда обязана исполниться не там, где живёт checkout.
//!
//! Проверяется запуск настоящего процесса: инструмент на Python исполняется ровно так,
//! как его будет исполнять packet-10. Ни одна acceptance-команда при этом НЕ исполняется —
//! для проверки песочницы используется только `git archive`.

use std::path::{Path, PathBuf};
use std::process::Command;

/// `xtask` лежит в корне репозитория, а не в `crates/`, поэтому подниматься на один
/// уровень. Лишний `parent` уводил путь за пределы workspace.
fn script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("scripts")
        .join("packet_replay.py")
}

fn run(args: &[&str]) -> (String, String, bool) {
    let out = Command::new("python3")
        .arg(script())
        .args(args)
        .output()
        .expect("инструмент должен запускаться");
    (
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
        out.status.success(),
    )
}

/// Песочница строится из настоящего `git archive HEAD`, поэтому проверяется на реальном
/// репозитории: подмена песочницы фикстурой ничего бы не доказала.
fn make_sandbox() -> PathBuf {
    let (stdout, stderr, ok) = run(&["replay", "--sandbox-only"]);
    assert!(ok, "построение песочницы обязано удасться: {}", stderr);
    stdout
        .lines()
        .find_map(|l| l.strip_prefix("SANDBOX="))
        .map(|l| PathBuf::from(l.trim()))
        .expect("инструмент обязан напечатать путь песочницы")
}

/// Проверка с уборкой: упавший assert не должен оставить песочницу в /tmp.
fn with_sandbox(f: impl FnOnce(&Path)) {
    let dir = make_sandbox();
    f(&dir);
    let _ = std::fs::remove_dir_all(&dir);
}

/// В песочнице нет `.git`: иначе команда может увидеть историю и записать в неё.
#[test]
fn sandbox_has_no_git_directory() {
    with_sandbox(|dir| {
        assert!(
            !dir.join(".git").exists(),
            "в песочнице не должно быть .git: {}",
            dir.display()
        );
    });
}

/// Песочница содержит дерево репозитория, а не пустой каталог.
#[test]
fn sandbox_contains_tracked_files() {
    with_sandbox(|dir| {
        assert!(
            dir.join("Cargo.toml").is_file(),
            "в песочнице должен быть корень workspace: {}",
            dir.display()
        );
        assert!(
            dir.join("crates").is_dir(),
            "в песочнице должен быть каталог крейтов: {}",
            dir.display()
        );
    });
}

/// Песочница — отдельный каталог, а не ссылка на рабочее дерево.
#[test]
fn sandbox_is_a_real_directory_not_a_symlink() {
    with_sandbox(|dir| {
        assert!(
            !dir.is_symlink(),
            "песочница не должна быть ссылкой: {}",
            dir.display()
        );
        assert!(
            dir.is_dir(),
            "песочница должна быть каталогом: {}",
            dir.display()
        );
    });
}

/// Песочница лежит вне рабочего дерева: иначе прогон способен испортить исходники.
#[test]
fn sandbox_lives_outside_the_working_tree() {
    with_sandbox(|dir| {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .canonicalize()
            .expect("репозиторий");
        assert!(
            !dir.starts_with(&repo),
            "песочница {} оказалась внутри репозитория {}",
            dir.display(),
            repo.display()
        );
    });
}
