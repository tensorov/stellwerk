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

/// Поля верхнего уровня `outcomes.json`, нужные гейту, в фиксированном порядке.
const COUNTER_KEYS: [&str; 5] = ["total_mutants", "caught", "missed", "timeout", "unviable"];

/// Четыре счётчика и `total_mutants` верхнего уровня `outcomes.json`. Берутся с
/// верхнего уровня, а не из `summary`: текст между версиями меняется, числа — нет.
struct Counters {
    total: u64,
    caught: u64,
    missed: u64,
    timeout: u64,
    unviable: u64,
}

impl Counters {
    /// `Err` — измерения не состоялось. `Ok(None)` — ни одного viable мутанта: это
    /// отдельное состояние (N/A), а не ноль процентов, потому что ноль означал бы
    /// провал тестов, а не отсутствие измерения.
    fn check(self) -> Result<Option<Self>, &'static str> {
        if self.caught + self.missed + self.timeout + self.unviable != self.total {
            return Err("сумма счётчиков не сошла с total_mutants");
        }
        // Позитивный контроль: ловимые мутанты есть, а поймано ноль — значит
        // непонятно, тесты не убивают мутанты или инструмент не собрал их в scratch.
        if self.caught == 0 && self.missed + self.timeout > 0 {
            return Err("позитивный контроль не пройден: viable есть, поймано 0");
        }
        Ok((self.caught + self.missed + self.timeout > 0).then_some(self))
    }
}

/// Счётчики верхнего уровня `outcomes.json`. Вложенные значения пропускаются насквозь,
/// поэтому счётчик из вложенного объекта в счёт верхнего уровня не попадёт. Зависимостей
/// у xtask нет намеренно, а контракт — пять целых полей, поэтому разбор минимальный;
/// `None` значит «поля нет или оно не число», и оба случая — «файл не распарсился».
fn counters(text: &str) -> [Option<u64>; 5] {
    let bytes = text.as_bytes();
    let mut found = [None; 5];
    let (mut depth, mut i) = (0i32, 0usize);
    let mut key: Option<&[u8]> = None;
    while let Some(&byte) = bytes.get(i) {
        i += 1;
        match byte {
            b'{' | b'[' => depth += 1,
            b'}' | b']' if depth > 0 => depth -= 1,
            b'"' => {
                let end = string_end(bytes, i - 1);
                // Ключ — только строка, за которой стоит `:`. Без этой проверки
                // строковое ЗНАЧЕНИЕ (например cargo_mutants_version: "27.1.0")
                // принималось за имя ключа, и следующий за ним счётчик терял имя.
                let mut after = end;
                while matches!(bytes.get(after), Some(w) if w.is_ascii_whitespace()) {
                    after += 1;
                }
                if depth == 1 && key.is_none() && bytes.get(after) == Some(&b':') {
                    key = Some(&bytes[i..end - 1]);
                }
                i = end;
            }
            b':' if depth == 1 => {
                while matches!(bytes.get(i), Some(w) if w.is_ascii_whitespace()) {
                    i += 1;
                }
                if let Some(k) = key.take() {
                    if let Some(at) = COUNTER_KEYS.iter().position(|c| c.as_bytes() == k) {
                        found[at] = number(&bytes[i..]);
                    }
                }
            }
            _ => {}
        }
    }
    found
}

/// Неотрицательное целое в начале среза. Дробное и `null` счётчиком не являются:
/// принять их за число значило бы превратить неизвестное в значение.
fn number(bytes: &[u8]) -> Option<u64> {
    let digits = bytes.iter().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 || matches!(bytes.get(digits), Some(b'.' | b'e' | b'E')) {
        return None;
    }
    std::str::from_utf8(&bytes[..digits]).ok()?.parse().ok()
}

/// Конец строкового литерала: индекс за закрывающей кавычкой, со счётом escape.
fn string_end(bytes: &[u8], mut i: usize) -> usize {
    i += 1;
    while let Some(&byte) = bytes.get(i) {
        i += 1;
        match byte {
            b'\\' => i += 1,
            b'"' => return i,
            _ => {}
        }
    }
    bytes.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = concat!(
        r#"{"cargo_mutants_version":"27.1.0","total_mutants":5,"caught":4,"#,
        r#""missed":1,"timeout":0,"unviable":0,"success":true}"#
    );

    fn counts(caught: u64, missed: u64, timeout: u64, unviable: u64) -> Counters {
        Counters {
            total: caught + missed + timeout + unviable,
            caught,
            missed,
            timeout,
            unviable,
        }
    }

    #[test]
    fn top_level_counters_are_read_in_declared_order() {
        assert_eq!(
            counters(GOOD),
            [Some(5), Some(4), Some(1), Some(0), Some(0)]
        );
    }

    #[test]
    fn nested_counter_is_not_counted_as_top_level() {
        let text = r#"{"summary":{"caught":999},"total_mutants":2,"caught":1,"missed":1,"timeout":0,"unviable":0}"#;
        assert_eq!(
            counters(text),
            [Some(2), Some(1), Some(1), Some(0), Some(0)]
        );
    }

    #[test]
    fn missing_and_non_integer_are_absent_not_zero() {
        let text = r#"{"total_mutants":1,"caught":null,"missed":1.5,"timeout":0}"#;
        let [total, caught, missed, _, _] = counters(text);
        assert_eq!((total, caught, missed), (Some(1), None, None));
    }

    #[test]
    fn sum_mismatch_is_refusal() {
        let wrong = Counters {
            total: 9,
            caught: 4,
            missed: 1,
            timeout: 0,
            unviable: 0,
        };
        assert_eq!(
            wrong.check().err(),
            Some("сумма счётчиков не сошла с total_mutants")
        );
    }

    #[test]
    fn viable_without_caught_is_refusal() {
        assert_eq!(
            counts(0, 3, 0, 0).check().err(),
            Some("позитивный контроль не пройден: viable есть, поймано 0")
        );
    }

    #[test]
    fn zero_viable_is_its_own_state_not_zero_percent() {
        assert_eq!(
            counts(0, 0, 0, 4).check().map(|got| got.is_none()),
            Ok(true)
        );
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
