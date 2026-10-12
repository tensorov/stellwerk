//! Спецификация N1-03: `SandboxWorker` — единственный production-путь запуска.
//!
//! Воркер обязан провести движок через песочницу и адаптер, а не мимо них. Здесь
//! проверяется, что успешный запуск возвращает вывод, а ненулевой выход и убийство
//! сигналом — отказ, а не тихий успех: упавший прогон, отчитавшийся `Succeeded`,
//! неотличим от состоявшегося (закон 6).

use std::path::{Path, PathBuf};
use std::time::Duration;

use stellwerk_core::contract::{DataClass, ModelBinding, RetentionClass, RiskClass};
use stellwerk_gateway::permission::ToolRisk;
use stellwerk_task::m5::sandbox_bwrap::Sandbox;
use stellwerk_task::m5::FixtureEngine;
use stellwerk_task::orchestrator::SandboxWorker;
use stellwerk_task::runner::{Call, ExecOutcome, Worker, WorkerError};

const WINDOW: Duration = Duration::from_secs(60);
const BUDGET: Duration = Duration::from_secs(30);

fn system_dirs() -> Vec<PathBuf> {
    ["/usr", "/bin", "/lib", "/lib64", "/etc"]
        .iter()
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .collect()
}

fn in_system(names: &[&str]) -> PathBuf {
    names
        .iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
        .unwrap_or_else(|| panic!("нет ни одного из {names:?}"))
}

fn fixture_root(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("sw-n1-03-worker-{tag}-{}", std::process::id()))
}

fn call() -> Call {
    Call {
        tool: "echo".to_string(),
        module: "stellwerk-task".to_string(),
        risk: ToolRisk::Classified(RiskClass::R0),
        data: DataClass::Public,
        model: ModelBinding {
            id: "fixture".to_string(),
            provider: "local".to_string(),
            model_id: "echo".to_string(),
            data_class: DataClass::Public,
            retention: RetentionClass::FirstParty,
            context_window: 1_000,
            max_output: 1_000,
            budget_tokens_per_task: 1_000,
        },
        interactive: false,
    }
}

fn worker(root: &Path, program: PathBuf, args: Vec<String>) -> SandboxWorker<FixtureEngine> {
    let sandbox = Sandbox {
        work: root.join("work"),
        tmp: root.join("tmp"),
        read_only: system_dirs(),
        window: WINDOW,
    };
    std::fs::create_dir_all(&sandbox.work).expect("work");
    std::fs::create_dir_all(&sandbox.tmp).expect("tmp");
    SandboxWorker::new(
        FixtureEngine::new("/bin/true"),
        sandbox,
        program,
        args,
        BUDGET,
        Vec::new(),
    )
}

fn run(worker: &SandboxWorker<FixtureEngine>) -> Result<ExecOutcome, WorkerError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(worker.exec("workspace", &call()))
}

#[test]
fn the_worker_runs_the_engine_inside_the_sandbox_and_returns_its_output() {
    let root = fixture_root("output");
    let echo = in_system(&["/usr/bin/echo", "/bin/echo"]);
    let worker = worker(&root, echo, vec!["hello".to_string()]);

    let outcome = run(&worker).expect("успешный запуск");
    assert_eq!(outcome.output, "hello\n");
    assert_eq!(worker.spawned(), 1, "воркер запускали ровно раз");
}

#[test]
fn a_non_zero_exit_is_a_failure_and_not_a_success() {
    let root = fixture_root("nonzero");
    let false_bin = in_system(&["/usr/bin/false", "/bin/false"]);
    let worker = worker(&root, false_bin, Vec::new());

    match run(&worker) {
        Err(WorkerError(message)) => {
            assert!(
                message.contains("code 1"),
                "причина обязана называть код: {message:?}"
            );
        }
        Ok(outcome) => panic!("ненулевой выход — отказ, а не {outcome:?}"),
    }
    assert_eq!(worker.spawned(), 1, "упавший запуск тоже считается");
}

#[test]
fn a_process_killed_by_a_signal_is_a_failure_and_not_a_success() {
    let root = fixture_root("signal");
    let sh = in_system(&["/usr/bin/sh", "/bin/sh"]);
    let worker = worker(&root, sh, vec!["-c".to_string(), "kill -9 $$".to_string()]);

    match run(&worker) {
        Err(WorkerError(message)) => {
            assert!(!message.is_empty(), "причина обязана называть виновника");
        }
        Ok(outcome) => panic!("убийство сигналом — отказ, а не {outcome:?}"),
    }
}
