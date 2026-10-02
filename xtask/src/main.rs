//! xtask — захардкоженный перечень задач верификации.
//!
//! Зачем: `evidence_verify` в PolyForge принимает только узкий allowlist команд
//! семейства `cargo`. Bash-наборы (34 теста `scripts/worker/tests/run.sh`) и
//! python-линтер (`scripts/plan-lint.py`) через него не аттестуемы, и блок
//! остаётся `ModelClaimed` без `Verified` — см. F-08.
//!
//! Расширять allowlist до `bash` нельзя: команда с произвольными аргументами
//! исполнит что угодно, и allowlist перестанет быть allowlist'ом. Здесь та же
//! проблема решается иначе — не ослаблением, а переносом: xtask сам является
//! обычным крейтом workspace, под контролем версий и ревью.
//!
//! # Почему здесь нет `Command::new(из_аргумента)`
//!
//! xtask входит в доверенную базу: он мост к механизму, который запрещает модели
//! верифицировать саму себя. Если задача определяется аргументом, то
//! `cargo xtask worker-tests --exec=...` (или подставленный путь) вернёт
//! произвольное исполнение — то есть дыра, ради закрытия которой всё и
//! затевалось. Поэтому:
//!
//! * задача — **литерал** в `match`, не значение из аргумента;
//! * путь к набору — **константа** в исходнике;
//! * аргументы, кроме имени задачи, **не перенаправляются** в дочерний процесс;
//! * неизвестная задача — ненулевой код и список разрешённых имён.
//!
//! # Почему инвертированный RED-гейт живёт в отдельной таблице
//!
//! Приёмка RED-пакета инвертирована: успех — это когда тест **падает**. Но
//! `evidence_verify` считает ненулевой код возврата провалом инструмента, поэтому
//! подтвердить падение можно было только компиляцией (`--no-run`), а свойство
//! «тест падает» оставалось утверждением модели (F-19). `RED_GATES` даёт обёртку:
//! тест собрался И упал → `0`.
//!
//! Обёртка, которая подтверждает падение там, где тест проходит, опаснее, чем её
//! отсутствие, поэтому строки таблицы обязаны совпадать с бинарниками, которые
//! действительно красные. Таблица намеренно пер-бинарная: запись allowlist —
//! тройка с **фиксированным** вектором аргументов, и имя бинарника в него не
//! входит (`red-gate <имя>` невыразима). `TASKS` и `RED_GATES` не сливаются:
//! у них разный смысл кода, и общий флаг означал бы, что смысл теряется по дороге.

use std::io::Write;
use std::process::{Command, ExitCode};

/// Задача: как запустить — программа и аргументы, зашитые в исходник.
struct Task {
    program: &'static str,
    args: &'static [&'static str],
}

const TASKS: &[(&str, Task)] = &[
    (
        "worker-tests",
        Task {
            program: "bash",
            args: &["scripts/worker/tests/run.sh"],
        },
    ),
    (
        "plan-lint",
        Task {
            program: "python3",
            args: &["scripts/plan-lint.py"],
        },
    ),
    (
        "plan-lint-selftest",
        Task {
            program: "python3",
            args: &["scripts/plan-lint.py", "--self-test"],
        },
    ),
    (
        "gate",
        Task {
            program: "just",
            args: &["check"],
        },
    ),
    (
        "ledger-encoding",
        Task {
            program: "cargo",
            args: &["test", "-p", "xtask", "--test", "ledger_encoding"],
        },
    ),
    (
        "packet-replay",
        Task {
            program: "python3",
            args: &["scripts/packet_replay.py", "report", "--report"],
        },
    ),
    (
        // Пять наборов перечислены поимённо, а не одним префиксом: префикс выбрал бы
        // ещё и будущие наборы, и тогда задача меняла бы смысл вместе с ростом
        // тестов. Список закрыт здесь же, в коде, — иначе вызывающий выбирал бы, что
        // именно проверяется, а аттестуемость требует, чтобы список был в коде.
        "packet-replay-selftest",
        Task {
            program: "cargo",
            args: &[
                "test",
                "-p",
                "xtask",
                "--test",
                "packet_replay_selftest",
                "--test",
                "packet_replay_validate",
                "--test",
                "packet_replay_aggregate",
                "--test",
                "packet_replay_execute",
                "--test",
                "packet_replay_run",
            ],
        },
    ),
];

/// Инвертированный RED-гейт: имя задачи — литерал, значение — константный
/// вектор аргументов `cargo test`. `red-gate <имя>` невозможна: у записи
/// allowlist вектор аргументов фиксирован, и имя бинарника в него не входит.
const RED_GATES: &[(&str, &[&str])] = &[
    (
        "red-gate-session-contract",
        &[
            "test",
            "-p",
            "stellwerk-gateway",
            "--test",
            "session_contract",
        ],
    ),
    (
        "red-gate-session",
        &["test", "-p", "stellwerk-gateway", "--test", "session"],
    ),
    // Бинарник появится вместе с M3-11c. Строка нужна сейчас, чтобы цепочка
    // RED-пакетов была полной и allowlist не пришлось править задним числом;
    // до того бинарника гейт честно отказывает с «не собрался».
    (
        "red-gate-session-failure",
        &[
            "test",
            "-p",
            "stellwerk-gateway",
            "--test",
            "session_failure",
        ],
    ),
    // Собственный красный бинарь гейта: ломает F-26/F-31, где позитивная фикстура
    // одалживала чужой бинарь и протухала ровно в момент слияния.
    (
        "red-gate-red-fixture",
        &[
            "test",
            "-p",
            "xtask",
            "--features",
            "red-fixture",
            "--test",
            "red_fixture",
        ],
    ),
    // Единственная намеренно зелёная строка: негативная фикстура. Без неё
    // нечем доказать, что обёртка отличает падение от прохождения, то есть
    // что она не rubber stamp.
    (
        "red-gate-permission",
        &["test", "-p", "stellwerk-gateway", "--test", "permission"],
    ),
];

/// Коды отказа инвертированного гейта. Мелкие, заведомо ненулевые и не
/// совпадающие с кодами `cargo test` (1, 4, 101): усечение до `u8` не должно
/// превращать отказ в успех.
const BUILD_FAILED: u8 = 3;
const NOT_RED: u8 = 4;
const CRASHED: u8 = 5;

/// Что сделал дочерний `cargo`. Код `None` — убит сигналом: падение из-за SIGSEGV
/// не подтверждает RED-спецификацию, и принять его за RED хуже, чем отказать.
enum Outcome {
    Failed,
    Passed,
    Crashed,
}

/// Имена из обеих таблиц: подсказка собирается отсюда, чтобы новая таблица не
/// оказалась не discoverable, а имена не пришлось бы перечислять второй раз.
fn task_names() -> Vec<&'static str> {
    TASKS
        .iter()
        .map(|(name, _)| *name)
        .chain(RED_GATES.iter().map(|(name, _)| *name))
        .collect()
}

fn usage() {
    eprintln!(
        "задача не указана. доступные задачи: {}",
        task_names().join(", ")
    );
    eprintln!("внимание: аргументы задачи не перенаправляются в наборы");
}

/// Один шаг гейта: `cargo` с вектором строки таблицы плюс `extra`. Вывод
/// дочернего процесса уходит в stderr целиком — stdout обёртки обязан остаться
/// пустым, потому что аттестуется код возврата, а не текст.
fn step(args: &[&str], extra: &[&str]) -> Result<Outcome, std::io::Error> {
    let out = Command::new("cargo").args(args).args(extra).output()?;
    let mut err = std::io::stderr().lock();
    let _ = err.write_all(&out.stdout);
    let _ = err.write_all(&out.stderr);
    Ok(match out.status.code() {
        Some(0) => Outcome::Passed,
        Some(_) => Outcome::Failed,
        None => Outcome::Crashed,
    })
}

/// Инвертированный гейт: тест обязан и собраться, и упасть. Порядок обязателен —
/// подтверждать падение того, что не собралось, нечего.
fn red_gate(name: &str, args: &[&str]) -> ExitCode {
    match step(args, &["--no-run"]) {
        Ok(Outcome::Passed) => {}
        Ok(Outcome::Failed) => {
            eprintln!("{name:?}: не собрался — падение подтверждать нечем");
            return ExitCode::from(BUILD_FAILED);
        }
        Ok(Outcome::Crashed) | Err(_) => {
            eprintln!("{name:?}: не удалось выполнить `cargo test --no-run`");
            return ExitCode::from(CRASHED);
        }
    }
    match step(args, &[]) {
        Ok(Outcome::Failed) => ExitCode::from(0),
        Ok(Outcome::Passed) => {
            eprintln!("{name:?}: тест зелёный — RED-спецификация не держится");
            ExitCode::from(NOT_RED)
        }
        Ok(Outcome::Crashed) | Err(_) => {
            eprintln!("{name:?}: не удалось выполнить `cargo test`");
            ExitCode::from(CRASHED)
        }
    }
}

fn main() -> ExitCode {
    // Берём только имя задачи. Всё остальное намеренно игнорируется, а не
    // перенаправляется: иначе лишний флаг стал бы путём к произвольной команде.
    let requested = std::env::args().nth(1);

    let Some(name) = requested.as_deref() else {
        usage();
        return ExitCode::from(2);
    };

    // Имя задачи — литерал, выбирающий строку; в строку команды оно не
    // подставляется, поэтому подмена пути или аргументов невозможна.
    if let Some((_, task)) = TASKS.iter().find(|(n, _)| *n == name) {
        return match Command::new(task.program).args(task.args).status() {
            Ok(status) => match status.code() {
                // Код возврата набора пробрасывается как есть: `cargo xtask` не должен
                // превращать провал проверки в успех.
                Some(code) => ExitCode::from((code & 0xFF) as u8),
                None => {
                    eprintln!("набор прерван сигналом");
                    ExitCode::from(1)
                }
            },
            Err(err) => {
                eprintln!("не удалось запустить {:?}: {err}", task.program);
                ExitCode::from(127)
            }
        };
    }

    if let Some((_, args)) = RED_GATES.iter().find(|(n, _)| *n == name) {
        return red_gate(name, args);
    }

    // Пользовательский ввод не выводится и не подставляется в
    // подсказку «похожих» имён: иначе сам список подсказок становится
    // оружием (напр. подставить префикс, чтобы «выстрелить» в соседнюю).
    eprintln!("неизвестная задача: {name:?}");
    eprintln!("доступные задачи: {}", task_names().join(", "));
    ExitCode::from(2)
}
