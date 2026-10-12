//! N1-04: `RunRequest` несёт НАСТОЯЩУЮ песочницу (`m5::sandbox_bwrap`), а не битый
//! `ShellSandbox`, и эта песочница отвергает запуск ДО старта процесса.
//!
//! Прежняя версия подставляла подменную песочницу через переменную окружения и проверяла
//! второй тип, чей `prepare` гарантированно падал: `scripts/worker/sandbox.sh` не имеет
//! argv и печатал пустоту, а `parse_root` возвращал `None`. Подмена маскировала этот
//! дефект. Здесь нет ни переменной окружения, ни скрипта — только настоящий тип поля и
//! его настоящие отказы.

use std::path::{Path, PathBuf};
use std::time::Duration;

use stellwerk_core::contract::{DataClass, ModelBinding, RetentionClass, TaskLimits, TaskSpec};
use stellwerk_task::m5::sandbox_bwrap::{Sandbox, SandboxError};
use stellwerk_task::runner::RunRequest;

/// Окно больше бюджета: иначе отказ был бы про окно, а не про путь или историю.
const WINDOW: Duration = Duration::from_secs(60);
const BUDGET: Duration = Duration::from_secs(30);
/// Программа запуска: до `spawn` дело не доходит, поэтому важен лишь её путь.
const TRUE: &str = "/bin/true";

/// Временный каталог фикстуры: имя по тегу и pid, уборка на `Drop` — `temp` не засоряется.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let named = format!("sw-n1-04-{tag}-{}", std::process::id());
        let dir = std::env::temp_dir().join(named);
        std::fs::create_dir_all(&dir).expect("каталог фикстуры создан");
        Self(dir)
    }

    fn root(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn spec() -> TaskSpec {
    TaskSpec {
        id: "n1-04".to_owned(),
        goal: "мигрировать RunRequest на настоящую песочницу".to_owned(),
        workspace_ref: "ws-stellwerk".to_owned(),
        model_ref: "n1-04".to_owned(),
        packet: "RunRequest несёт m5::sandbox_bwrap::Sandbox".to_owned(),
        limits: TaskLimits {
            max_files: 3,
            max_diff_lines: 200,
            timeout_seconds: 900,
            max_retries: 0,
            context_budget_tokens: 40_000,
            data_class: DataClass::Internal,
        },
        evidence_policy: "cargo test -p stellwerk-task --test sandbox".to_owned(),
    }
}

fn binding() -> ModelBinding {
    ModelBinding {
        id: "n1-04".to_owned(),
        provider: "neuraldeep".to_owned(),
        model_id: "m".to_owned(),
        data_class: DataClass::Public,
        retention: RetentionClass::FirstParty,
        context_window: 200_000,
        max_output: 8_000,
        budget_tokens_per_task: 43_000,
    }
}

/// `RunRequest` с настоящей песочницей над каталогом фикстуры: каталоги создаёт
/// вызывающий, конструктор песочницы не пишет ничего.
fn request_at(root: &Path) -> RunRequest {
    let (work, tmp) = (root.join("work"), root.join("tmp"));
    std::fs::create_dir_all(&work).expect("рабочая копия создана");
    std::fs::create_dir_all(&tmp).expect("каталог временных файлов создан");
    RunRequest {
        spec: spec(),
        model: binding(),
        sandbox: Sandbox {
            work,
            tmp,
            read_only: Vec::new(),
            window: WINDOW,
        },
    }
}

/// Поле несёт настоящую песочницу: она собирает `bwrap`-команду, а не зовёт shell-скрипт.
#[test]
fn run_request_carries_the_real_sandbox_that_builds_a_bwrap_command_inside_the_working_copy() {
    let scratch = Scratch::new("migrated");
    let request = request_at(scratch.root());

    assert_eq!(
        request.sandbox.work,
        scratch.root().join("work"),
        "поле RunRequest — рабочая копия настоящей песочницы",
    );

    let built = request
        .sandbox
        .command(&request.sandbox.work, Path::new(TRUE), &[], BUDGET)
        .expect("внутри рабочей копии запуск разрешён");
    assert_eq!(
        built.get_program().to_string_lossy(),
        "bwrap",
        "команда — настоящий bwrap, а не `sh scripts/worker/sandbox.sh`",
    );
}

/// Настоящий отказ ДО запуска: `..` уводит путь записи за корень рабочей копии.
#[test]
fn run_request_sandbox_refuses_a_write_path_that_escapes_the_working_copy() {
    let scratch = Scratch::new("outside");
    let request = request_at(scratch.root());

    let escape = request.sandbox.work.join("../outside.txt");
    let refused = request
        .sandbox
        .command(&escape, Path::new(TRUE), &[], BUDGET)
        .expect_err("путь снаружи обязан быть отвергнут до запуска");
    match refused {
        SandboxError::Outside { path, .. } => {
            assert_eq!(
                path,
                scratch.root().join("outside.txt"),
                "назван нормализованный путь"
            );
        }
        other => panic!("ожидался Outside, а не {other:?}"),
    }
}

/// `.git` внутри — это клон, а не выгрузка состояния: отказ до запуска.
#[test]
fn run_request_sandbox_refuses_a_working_copy_that_holds_history() {
    let scratch = Scratch::new("history");
    let request = request_at(scratch.root());
    std::fs::create_dir_all(request.sandbox.work.join(".git")).expect("клон создан");

    let refused = request
        .sandbox
        .command(&request.sandbox.work, Path::new(TRUE), &[], BUDGET)
        .expect_err("копия с историей обязана быть отвергнута до запуска");
    match refused {
        SandboxError::History { path } => assert!(path.ends_with(".git"), "{path:?}"),
        other => panic!("ожидался History, а не {other:?}"),
    }
}
