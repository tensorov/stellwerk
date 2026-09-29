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
];

fn usage() {
    let names: Vec<&str> = TASKS.iter().map(|(n, _)| *n).collect();
    eprintln!("задача не указана. доступные задачи: {}", names.join(", "));
    eprintln!("внимание: аргументы задачи не перенаправляются в наборы");
}

fn main() -> ExitCode {
    // Берём только имя задачи. Всё остальное намеренно игнорируется, а не
    // перенаправляется: иначе лишний флаг стал бы путём к произвольной команде.
    let requested = std::env::args().nth(1);

    let task = match requested.as_deref() {
        None => {
            usage();
            return ExitCode::from(2);
        }
        Some(name) => match TASKS.iter().find(|(n, _)| *n == name) {
            Some((_, t)) => t,
            None => {
                // Пользовательский ввод не выводится и не подставляется в
                // подсказку «похожих» имён: иначе сам список подсказок становится
                // оружием (напр. подставить префикс, чтобы «выстрелить» в соседнюю).
                eprintln!("неизвестная задача: {name:?}");
                let names: Vec<&str> = TASKS.iter().map(|(n, _)| *n).collect();
                eprintln!("доступные задачи: {}", names.join(", "));
                return ExitCode::from(2);
            }
        },
    };

    match Command::new(task.program).args(task.args).status() {
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
    }
}
