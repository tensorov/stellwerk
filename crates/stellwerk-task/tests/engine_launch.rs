//! Спецификация N1-03: адаптер исполняет ПОДГОТОВЛЕННЫЙ sandbox-launch.
//!
//! До N1-03 адаптер собирал команду сам (`Command::new(program)`), и `bwrap` с
//! `env_clear` оставались декорацией: их можно было не позвать, и никто бы не заметил.
//! Здесь проверяется обратное: команду собирает песочница, адаптер её только исполняет,
//! а окружение задаётся до запуска и пусто, если вызывающий ничего не разрешил.
//!
//! Тесты идут на программах, которые в системе есть по определению (`echo`, `env`), и
//! требуют `bwrap`: без него изоляцию нечем доказать, и тест обязан упасть, а не
//! отчитаться зелёным на голом `Command::new`.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use stellwerk_task::m5::sandbox_bwrap::Sandbox;
use stellwerk_task::m5::{EngineAdapter, FixtureEngine};

const WINDOW: Duration = Duration::from_secs(60);
const BUDGET: Duration = Duration::from_secs(30);

/// Системные каталоги на чтение. Пустой список заставил бы `bwrap` не найти программу
/// (`execvp: No such file or directory`), и тест падал бы не по делу.
fn system_dirs() -> Vec<PathBuf> {
    ["/usr", "/bin", "/lib", "/lib64", "/etc"]
        .iter()
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .collect()
}

/// Первый существующий путь из списка. Тест не должен зависеть от того, где именно
/// лежит `echo` на этой машине.
fn in_system(names: &[&str]) -> PathBuf {
    names
        .iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
        .unwrap_or_else(|| panic!("нет ни одного из {names:?}"))
}

fn fixture_root(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("sw-n1-03-{tag}-{}", std::process::id()))
}

fn sandbox_at(root: &Path) -> Sandbox {
    Sandbox {
        work: root.join("work"),
        tmp: root.join("tmp"),
        read_only: system_dirs(),
        window: WINDOW,
    }
}

fn as_text(arg: &OsStr) -> String {
    arg.to_string_lossy().into_owned()
}

#[test]
fn the_adapter_executes_the_prepared_bwrap_launch() {
    let root = fixture_root("prepared");
    let sandbox = sandbox_at(&root);
    std::fs::create_dir_all(&sandbox.work).expect("work");
    std::fs::create_dir_all(&sandbox.tmp).expect("tmp");
    let echo = in_system(&["/usr/bin/echo", "/bin/echo"]);

    let mut launch = sandbox
        .command(&sandbox.work, &echo, &["hello"], BUDGET)
        .expect("запуск внутри рабочей копии разрешён");

    // Подготовленный launch — это bwrap-argv, а не голый `Command::new(program)`.
    let argv: Vec<String> = launch.get_args().map(as_text).collect();
    let dump = format!("{argv:?}");
    assert!(argv.contains(&"--unshare-net".to_owned()), "{dump}");
    assert!(argv.iter().any(|a| a == "--ro-bind"), "{dump}");
    assert_eq!(launch.get_program().to_string_lossy(), "bwrap", "{dump}");

    launch.env_clear();
    let outcome = FixtureEngine::new("/bin/true")
        .session(launch, BUDGET)
        .expect("сессия");
    assert_eq!(outcome.code, Some(0), "stderr: {:?}", outcome.stderr);
    assert_eq!(outcome.stdout, "hello\n");
}

#[test]
fn the_prepared_launch_runs_with_an_empty_environment() {
    let root = fixture_root("env");
    let sandbox = sandbox_at(&root);
    std::fs::create_dir_all(&sandbox.work).expect("work");
    std::fs::create_dir_all(&sandbox.tmp).expect("tmp");
    let env = in_system(&["/usr/bin/env", "/bin/env"]);

    let mut launch = sandbox
        .command(&sandbox.work, &env, &[], BUDGET)
        .expect("запуск разрешён");
    launch.env_clear();

    let outcome = FixtureEngine::new("/bin/true")
        .session(launch, BUDGET)
        .expect("сессия");
    assert_eq!(outcome.code, Some(0), "stderr: {:?}", outcome.stderr);
    // `env_clear` снял унаследованное окружение. `bwrap` сам выставляет `PWD` по
    // `--chdir` — это единственное, что вправе остаться; любая другая строка
    // означала бы, что окружение родителя протекло в песочницу.
    let leaked: Vec<&str> = outcome
        .stdout
        .lines()
        .filter(|line| !line.starts_with("PWD="))
        .collect();
    assert!(
        leaked.is_empty(),
        "унаследованное окружение протекло: {leaked:?}"
    );
}

#[test]
fn a_process_killed_by_a_signal_has_no_exit_code_and_is_not_a_success() {
    let mut kill = Command::new("/bin/sh");
    kill.args(["-c", "kill -9 $$"]);

    let outcome = FixtureEngine::new("/bin/true")
        .session(kill, Duration::from_secs(5))
        .expect("сессия");

    assert_eq!(
        outcome.code, None,
        "убитый сигналом процесс не имеет кода завершения"
    );
}
