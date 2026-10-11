//! Интеграционные тесты CLI xtask: проверяется код возврата, а не текст.
//!
//! Зачем отдельный файл, а не тест в `main.rs`: `main.rs` — доверенная база, и
//! предикат, живущий в ней же, не может проверять саму обёртку.
//!
//! Коды утверждаются как «ненулевой», а не конкретное число: `cargo test`
//! возвращает 101, 1 или 4 в зависимости от того, чем именно сломалось. Поэтому
//! отказ опознаётся по маркеру из stderr, взятому дословно из ветки отказа
//! `main.rs`, и не зависит от того, какое число `cargo` выбрал на этой неделе.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Строка таблицы, на которой держится весь позитивный смысл сьютки.
///
/// Раньше здесь жило имя чужой задачи (`red-gate-session-failure`), красной
/// только пока шла цепочка RED-пакетов, и оно протухало ровно в момент слияния:
/// F-26 на `session_contract`, затем F-31 на `session_failure`. Сейчас это
/// собственный прибор гейта: красный всегда, независимо от того, какой пакет
/// сейчас на середине.
const POSITIVE_FIXTURE: &str = "red-gate-red-fixture";

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
            "прибор {POSITIVE_FIXTURE:?} отрапортовал себя зелёным — он обязан падать всегда. \
             Ослаблять его assert нельзя: гейт тогда сертифицирует прохождение, а обёртка, \
             подтверждающая успех, хуже отсутствия обёртки. Причина отказа: {stderr}"
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
fn red_fixture_instrument_is_red() {
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

/// Тест живёт здесь, а не в `tests/ledger_encoding.rs`, потому что задача сама запускает
/// `cargo test -p xtask --test ledger_encoding`. Проверка «задача существует», поставленная
/// внутрь проверяемого бинарника, даёт рекурсию: задача запустила бы прогон, который запустил
/// бы задачу. Симптом — зависание на семафоре cargo, а не падение.
#[test]
fn ledger_encoding_task_is_known_and_passes() {
    let out = run(&["ledger-encoding"]);
    assert_not_unknown(&out, "ledger-encoding");
    assert_eq!(
        code(&["ledger-encoding"]),
        0,
        "задача обязана заканчиваться успехом: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

/// Измерение целиком подменять нечем, а настоящий `cargo mutants` в тесте — минуты замера.
/// Подменяется ТОЛЬКО инструмент: разбор артефакта, путь и код возврата — настоящие.
const STUB: &str = r#"#!/bin/sh
out=
prev=
for a in "$@"; do
  if [ "$prev" = "--output" ]; then out=$a; fi
  prev=$a
done
printf '%s\n' "$*" > "$STUB_DIR/argv"
if [ -f "$STUB_DIR/outcomes.json" ]; then
  mkdir -p "$out/mutants.out"
  cp "$STUB_DIR/outcomes.json" "$out/mutants.out/outcomes.json"
fi
echo "mutants: строка дочернего процесса в stdout"
echo "mutants: строка дочернего процесса в stderr" >&2
exit CODETAIL
"#;

/// Временный каталог фикстуры: он же рабочий каталог xtask, поэтому артефакт пишется
/// внутрь песочницы. Счётчик обязателен: тесты идут параллельно и стёрли бы артефакты.
fn sandbox() -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("xtask-mutation-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("каталог фикстуры создаётся");
    dir
}

/// Фикстура артефакта cargo-mutants 27.1.0: пять счётчиков верхнего уровня плюс вложенный
/// объект внутри `outcomes`, чтобы разбор верхнего уровня был проверен на чужом поле.
fn outcomes(total: u64, caught: u64, missed: u64, timeout: u64, unviable: u64) -> String {
    format!(
        r#"{{"outcomes": [{{"scenario": {{"Build": {{"process_status": "Success"}}}},
             "summary": "Unviable"}}],
             "total_mutants": {total}, "caught": {caught}, "missed": {missed},
             "timeout": {timeout}, "unviable": {unviable}, "success": 0,
             "cargo_mutants_version": "27.1.0"}}"#
    )
}

fn run_with_stub(args: &[&str], dir: &Path, artifact: &str, tool_exit: i32) -> Output {
    use std::os::unix::fs::PermissionsExt;
    // Заглушка зовётся `cargo-mutants`, а не `cargo`: имя программы ищется по PATH
    // процесса xtask, который не наследует подменённый PATH, а вот `cargo`, получив
    // его в своём окружении, ищет подкоманду `cargo-mutants` уже в нём.
    let stub = dir.join("cargo-mutants");
    let body = STUB.replace("CODETAIL", &tool_exit.to_string());
    std::fs::write(&stub, body).expect("заглушка пишется");
    let mode = std::fs::Permissions::from_mode(0o755);
    std::fs::set_permissions(&stub, mode).expect("заглушка исполняемая");
    if !artifact.is_empty() {
        let fixture = dir.join("outcomes.json");
        std::fs::write(fixture, artifact).expect("фикстура пишется");
    }
    let host = std::env::var("PATH").unwrap_or_default();
    let path = format!("{}:{host}", dir.display());
    Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(args)
        .current_dir(dir)
        .env("PATH", path)
        .env("STUB_DIR", dir)
        .output()
        .unwrap_or_else(|err| panic!("не удалось запустить xtask {args:?}: {err}"))
}

/// Код возврата. Отсутствие кода означало бы, что процесс убит сигналом, и это не гейт.
fn code_of(out: &Output) -> i32 {
    out.status.code().expect("xtask не убит сигналом")
}

/// Измерение состоялось, даже когда инструмент вернул 1: это вердикт, а не поломка.
/// Числа — замер 2026-10-04 по stellwerk-gateway (269 / 71 / 177 / 0 / 21 = 28.63%),
/// то есть заведомо ниже порога 80.
#[test]
fn mutation_run_exits_zero_on_fail_verdict() {
    let dir = sandbox();
    let artifact = outcomes(269, 71, 177, 0, 21);
    let out = run_with_stub(&["mutation-run"], &dir, &artifact, 1);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(code_of(&out), 0, "{stderr}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.is_empty(), "stdout обёртки пуст: {stdout}");
}

/// Нет артефакта — измерения не было: код 0 означал бы запись о невыполненной работе.
#[test]
fn mutation_run_refuses_unfinished_measurement() {
    let dir = sandbox();
    let out = run_with_stub(&["mutation-run"], &dir, "", 1);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_ne!(code_of(&out), 0, "{stderr}");
    assert!(stderr.contains("нет outcomes.json"), "{stderr}");
}
/// Ровно на пороге — PASS: сравнение целочисленное, а «ровно 80%» не должно
/// превращаться в FAIL из-за 80.00000000000001 в f64. missed=0: при missed>0
/// срабатывает второй clause, и 80% уже не проходит.
#[test]
fn mutation_tasks_pass_exactly_at_threshold() {
    let dir = sandbox();
    let out = run_with_stub(&["mutation-task"], &dir, &outcomes(5, 4, 0, 1, 0), 2);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(code_of(&out), 0, "{stderr}");
    assert!(stderr.contains("PASS"), "PASS не назван: {stderr}");
    let argv = std::fs::read_to_string(dir.join("argv")).expect("argv пишет заглушка");
    assert!(
        argv.contains("stellwerk-task"),
        "измерен чужой крейт: {argv}"
    );
}

/// Ниже порога вердикт кодируется ненулевым кодом — в отличие от `mutation-run`, где
/// то же измерение с FAIL даёт 0.
#[test]
fn mutation_tasks_fail_below_threshold() {
    let dir = sandbox();
    let out = run_with_stub(&["mutation-task"], &dir, &outcomes(100, 79, 21, 0, 0), 2);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_ne!(code_of(&out), 0, "79% ниже порога 80: {stderr}");
    assert!(stderr.contains("FAIL"), "отказ не назван FAIL: {stderr}");
}

/// Ноль viable — отдельное состояние: код 0 и вердикт N/A, а не PASS. «Нет ловимых
/// мутантов» и «тесты ничего не ловят» — не одно и то же.
#[test]
fn mutation_tasks_report_not_applicable() {
    let dir = sandbox();
    let out = run_with_stub(&["mutation-gateway"], &dir, &outcomes(5, 0, 0, 0, 5), 2);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(code_of(&out), 0, "{stderr}");
    assert!(stderr.contains("N/A"), "N/A не назван: {stderr}");
    assert!(!stderr.contains("PASS"), "N/A выдан за PASS: {stderr}");
}

/// Лишний флаг не должен доходить до инструмента: имя задачи — литерал в `match`,
/// а из `argv` в дочерний процесс не уходит ничего.
#[test]
fn mutation_tasks_do_not_forward_arguments() {
    let dir = sandbox();
    let args = ["mutation-task", "--exec=/bin/false"];
    // Проверяется пересылка argv, а не вердикт: нужен PASS, поэтому missed=0.
    let out = run_with_stub(&args, &dir, &outcomes(5, 4, 0, 1, 0), 2);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(code_of(&out), 0, "лишний флаг ломает задачу: {stderr}");
    let argv = std::fs::read_to_string(dir.join("argv")).expect("argv пишет заглушка");
    let want = "mutants --package stellwerk-task --output target/mutants/stellwerk-task";
    assert_eq!(argv.trim(), want, "в дочерний процесс ушло лишнее: {argv}");
}

/// missed > 0 должен блокировать коммит — даже при 100% пойманных viable.
#[test]
fn mutation_tasks_fail_when_missed_gt_zero() {
    let dir = sandbox();
    // caught=4, missed=1, timeout=0, unviable=0, total=5: 100% viable поймано, но missed>0
    let out = run_with_stub(&["mutation-task"], &dir, &outcomes(5, 4, 1, 0, 0), 2);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_ne!(code_of(&out), 0, "missed>0 должен блокировать: {stderr}");
    assert!(stderr.contains("FAIL"), "отказ не назван FAIL: {stderr}");
    assert!(
        stderr.contains("missed>0"),
        "должно упоминать missed>0: {stderr}"
    );
}

/// missed == 0 — clause не блокирует; процентный порог остаётся решающим.
#[test]
fn mutation_tasks_missed_zero_does_not_block_by_itself() {
    let dir = sandbox();
    // 79% ниже порога 80, missed==0 — блокирует именно процентный порог
    let out = run_with_stub(&["mutation-task"], &dir, &outcomes(100, 79, 0, 21, 0), 2);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_ne!(
        code_of(&out),
        0,
        "должен блокировать по порогу 80: {stderr}"
    );
    assert!(stderr.contains("FAIL"), "отказ не назван FAIL: {stderr}");
}

/// missed == 0 и порог пройден — PASS.
#[test]
fn mutation_tasks_missed_zero_passes_when_threshold_ok() {
    let dir = sandbox();
    let out = run_with_stub(&["mutation-task"], &dir, &outcomes(5, 4, 0, 1, 0), 2);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(code_of(&out), 0, "{stderr}");
    assert!(stderr.contains("PASS"), "PASS не назван: {stderr}");
}

/// Отсутствие данных — не ноль. Если missed отсутствует, измерение должно быть отвергнуто.
#[test]
fn mutation_tasks_refuse_when_missed_missing() {
    let dir = sandbox();
    let artifact = r#"{"outcomes": [],
             "total_mutants": 5, "caught": 5, "timeout": 0, "unviable": 0, "success": 0,
             "cargo_mutants_version": "27.1.0"}"#;
    let out = run_with_stub(&["mutation-task"], &dir, artifact, 2);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_ne!(
        code_of(&out),
        0,
        "отсутствующий missed должен отвергаться: {stderr}"
    );
    assert!(stderr.contains("не распарсился"), "{stderr}");
}
