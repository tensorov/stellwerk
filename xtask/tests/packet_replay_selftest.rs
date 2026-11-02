//! Self-tests для парсера packet-файлов (`packet-08`).
//!
//! Каждый тест берёт известный вход и проверяет известный выход. Это и есть смысл
//! инструмента: предыдущие три ложные находки выросли из конвейеров, собранных в
//! моменте, — они не ловили собственный баг, потому что у них не было эталона.
//!
//! Тесты НЕ запускают ни одной acceptance-команды: этот пакет только разбирает.

use std::path::{Path, PathBuf};
use std::process::Command;

fn script() -> PathBuf {
    // `xtask` лежит в корне репозитория, а не в `crates/`, поэтому подниматься
    // на один уровень, а не на два: лишний `parent` уводил путь за пределы
    // workspace, и скрипт «не существовал» при реально существующем файле.
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

fn fixture() -> PathBuf {
    // Каталог свой на каждый тест: имя берётся из имени потока, который cargo даёт
    // тесту. Общий каталог означал бы, что тесты, идущие параллельно, удаляют
    // фикстуры друг друга прямо в момент прогона — наблюдалось как «скрипт работает,
    // а три теста падают», при верном выводе при одиночном запуске.
    let tag = std::thread::current()
        .name()
        .unwrap_or("unnamed")
        .replace("::", "-");
    let dir = std::env::temp_dir().join(format!("stellwerk-replay-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    let plans = dir.join(".sisyphus/plans");
    std::fs::create_dir_all(&plans).expect("каталог фикстур");
    dir
}

/// Минимальный пакет: всё, что парсер обязан вытащить.
fn write_packet(dir: &Path, milestone: &str, name: &str, body: &str) {
    let p = dir.join(format!(".sisyphus/plans/{milestone}")).join(name);
    std::fs::create_dir_all(p.parent().unwrap()).expect("milestone dir");
    std::fs::write(&p, body).expect("пакет записан");
}

/// 1. Однострочное `test: cargo test …` разбирается.
#[test]
fn single_line_test_field_is_parsed() {
    let dir = fixture();
    write_packet(
        &dir,
        "M1",
        "packet-01.md",
        "# packet-01\nplan_id: M1\ngoal: GREEN — реализовать X\ntest: cargo test -p stellwerk-core --test task_lifecycle\nforbidden: [cut]\n",
    );
    let (stdout, _, ok) = run(&["inventory", "--root", dir.to_str().unwrap()]);
    assert!(ok, "инвентарь должен строиться: {}", stdout);
    assert!(stdout.contains("PARSED"), "ожидался PARSED: {}", stdout);
    assert!(
        stdout.contains("task_lifecycle"),
        "команда должна быть извлечена: {}",
        stdout
    );
}

/// 2. Команда, обёрнутая в backticks, разбирается.
#[test]
fn backticked_command_is_parsed() {
    let dir = fixture();
    write_packet(
        &dir,
        "M1",
        "packet-02.md",
        "# packet-02\ngoal: GREEN\ntest: `cargo test -p stellwerk-store --test task_crud`\n",
    );
    let (stdout, _, ok) = run(&["inventory", "--root", dir.to_str().unwrap()]);
    assert!(ok, "{}", stdout);
    assert!(stdout.contains("task_crud"), "backticks сняты: {}", stdout);
    assert!(
        !stdout.contains('`'),
        "backtik не должны попасть в значение"
    );
}

/// 3. Слово `test` в прозе и в code block полем НЕ считается.
///    Это главная защита от ложного срабатывания: искать поле по вхождению слова
///    «test» значит найти его в любой фразе вроде «прогнать этот test».
#[test]
fn prose_and_code_block_mentions_are_not_fields() {
    let dir = fixture();
    write_packet(
        &dir,
        "M1",
        "packet-03.md",
        "# packet-03\ngoal: GREEN\n  сначала прогоним этот test, потом test: не поле\n\n```text\ntest: cargo test --features x\n```\n  и снова test вне поля\n",
    );
    let (stdout, _, ok) = run(&["inventory", "--root", dir.to_str().unwrap()]);
    assert!(ok, "{}", stdout);
    assert!(
        stdout.contains("NO_COMMAND"),
        "ни одно из упоминаний не поле — ожидался NO_COMMAND: {}",
        stdout
    );
    assert!(
        !stdout.contains("features"),
        "команда из code block не должна извлекаться: {}",
        stdout
    );
}

/// 4. Пакет без `test:` → NO_COMMAND.
#[test]
fn packet_without_test_field_is_no_command() {
    let dir = fixture();
    write_packet(
        &dir,
        "M1",
        "packet-04.md",
        "# packet-04\ngoal: только цель\n",
    );
    let (stdout, _, ok) = run(&["inventory", "--root", dir.to_str().unwrap()]);
    assert!(ok, "{}", stdout);
    assert!(
        stdout.contains("NO_COMMAND"),
        "ожидался NO_COMMAND: {}",
        stdout
    );
}

/// 5. `_archive/` не попадает в обход.
#[test]
fn archive_is_not_scanned() {
    let dir = fixture();
    write_packet(
        &dir,
        "M1",
        "packet-05.md",
        "# packet-05\ngoal: живой\ntest: cargo test -p stellwerk-core\n",
    );
    let archived = dir.join(".sisyphus/plans/M1/_archive/packet-99.md");
    std::fs::create_dir_all(archived.parent().unwrap()).unwrap();
    std::fs::write(
        &archived,
        "# packet-99\ntest: cargo test --features archived_marker\n",
    )
    .unwrap();

    let (stdout, _, ok) = run(&["inventory", "--root", dir.to_str().unwrap()]);
    assert!(ok, "{}", stdout);
    assert!(
        !stdout.contains("archived_marker"),
        "_archive обязан быть пропущен: {}",
        stdout
    );
    assert!(
        !stdout.contains("packet-99"),
        "_archive не должен попасть в счёт: {}",
        stdout
    );
}

/// 6. `.omo/plans/` не попадает в обход — это зеркало, а не источник истины.
#[test]
fn omo_mirror_is_not_scanned() {
    let dir = fixture();
    write_packet(
        &dir,
        "M1",
        "packet-06.md",
        "# packet-06\ngoal: канон\ntest: cargo test -p stellwerk-core\n",
    );
    let mirror = dir.join(".omo/plans/M1/packet-06.md");
    std::fs::create_dir_all(mirror.parent().unwrap()).unwrap();
    std::fs::write(
        &mirror,
        "# packet-06\ntest: cargo test --features omo_marker\n",
    )
    .unwrap();

    let (stdout, _, ok) = run(&["inventory", "--root", dir.to_str().unwrap()]);
    assert!(ok, "{}", stdout);
    assert!(
        !stdout.contains("omo_marker"),
        "зеркало .omo/plans обязано игнорироваться: {}",
        stdout
    );
}

/// 7. Обрезанное имя сохраняется полностью. Ровно тот случай, который породил ложную
///    находку про M1: `cut -c1:44` превращал `workspace_policy` в `workspace`.
#[test]
fn long_target_name_is_not_truncated() {
    let dir = fixture();
    let long = "cargo test -p stellwerk-core --test workspace_policy_routing_extended";
    write_packet(
        &dir,
        "M1",
        "packet-07.md",
        &format!("# packet-07\ngoal: длинное имя\ntest: {long}\n"),
    );
    let (stdout, _, ok) = run(&["inventory", "--root", dir.to_str().unwrap()]);
    assert!(ok, "{}", stdout);
    assert!(
        stdout.contains("workspace_policy_routing_extended"),
        "имя обязано сохраниться целиком: {}",
        stdout
    );
    assert!(
        !stdout.contains("workspace_policy\n"),
        "хвост имени не должен теряться при печати: {}",
        stdout
    );
}

/// 8. Инструмент не использует `cut`, shell word-splitting и zsh glob.
///    Проверяется чтением исходника, а не декларацией: запрещённый приём в коде
///    ловится только так.
#[test]
fn source_uses_no_cut_and_no_shell() {
    let src = std::fs::read_to_string(script()).expect("исходник читается");
    for banned in ["cut ", "|cut", "| cut", "shell=True", "eval(", "os.system"] {
        assert!(
            !src.contains(banned),
            "запрещённый приём `{banned}` в исходнике инструмента"
        );
    }
}

/// 9. Числа печатаются из посчитанного, а не подставлены: состав вывода должен
///    отражать содержимое фикстуры, а не ориентиры брифа.
#[test]
fn counts_match_fixture_not_brief_heuristics() {
    let dir = fixture();
    for i in 1..=3 {
        write_packet(
            &dir,
            "M1",
            &format!("packet-1{i}.md"),
            "# packet\ngoal: g\ntest: cargo test -p stellwerk-core\n",
        );
    }
    write_packet(&dir, "M2", "packet-01.md", "# packet\ngoal: без команды\n");

    let (stdout, _, ok) = run(&["inventory", "--root", dir.to_str().unwrap()]);
    assert!(ok, "{}", stdout);
    assert!(
        stdout.contains("PARSED=3"),
        "три пакета с командой: {}",
        stdout
    );
    assert!(
        stdout.contains("NO_COMMAND=1"),
        "один без команды: {}",
        stdout
    );
    assert!(
        !stdout.contains("96") && !stdout.contains("89"),
        "ориентиры брифа не должны попадать в вывод: {}",
        stdout
    );
}

/// 10. M1 и M3 пакеты не фильтруются — сканируются как все остальные.
#[test]
fn m1_and_m3_packets_are_not_filtered() {
    let dir = fixture();
    write_packet(
        &dir,
        "M1",
        "packet-05.md",
        "# packet-05\ngoal: M1 пакет\ntest: cargo test -p stellwerk-core --test m1_case\n",
    );
    write_packet(
        &dir,
        "M3",
        "packet-01.md",
        "# packet-01\ngoal: M3 пакет\ntest: cargo test -p stellwerk-store --test m3_case\n",
    );

    let (stdout, _, ok) = run(&["inventory", "--root", dir.to_str().unwrap()]);
    assert!(ok, "инвентарь должен строиться: {}", stdout);
    assert!(
        stdout.contains("m1_case"),
        "M1/packet-05 должен быть в инвентаре: {}",
        stdout
    );
    assert!(
        stdout.contains("m3_case"),
        "M3/packet-01 должен быть в инвентаре: {}",
        stdout
    );
}
