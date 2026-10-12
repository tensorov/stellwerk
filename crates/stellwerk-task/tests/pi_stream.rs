//! N1-05: stdout прогона обязан стать ИЗМЕРЕННЫМ `RunOutcome`, а не непрозрачным blob.
//!
//! `Worker::exec` отдаёт лишь текст (`ExecOutcome`), а `evaluate()` требует `RunOutcome` с
//! `verdict`. Годность считается ТОЛЬКО из Pi-событий — `Progress::absorb` +
//! `verdict_for` — поэтому оркестратор обязан разобрать поток. `/bin/echo` не печатает
//! событий: пустой поток обязан давать `verdict != Valid`, и это корректное поведение,
//! а не сбой. Пороги берутся из раннера (`GATE_REQUIRE_*`); своих чисел тест не вводит.

use std::path::{Path, PathBuf};
use std::time::Duration;

use stellwerk_core::contract::{DataClass, ModelBinding, RetentionClass, RiskClass};
use stellwerk_gateway::permission::ToolRisk;
use stellwerk_task::m5::sandbox_bwrap::Sandbox;
use stellwerk_task::m5::FixtureEngine;
use stellwerk_task::orchestrator::SandboxWorker;
use stellwerk_task::runner::{
    Call, ExecOutcome, RunOutcome, StopReason, Verdict, Worker, WorkerError,
};

const WINDOW: Duration = Duration::from_secs(60);
const BUDGET: Duration = Duration::from_secs(30);
const FIXTURE: &str = "../stellwerk-cli/examples/pi-fixture.sh";

const NO_TOOL: &str = r#"cat <<'EOF'
{"type":"message_end","message":{"role":"assistant","stopReason":"toolUse"}}
{"type":"message_end","message":{"role":"assistant","stopReason":"stop"}}
{"type":"turn_end","message":{"stopReason":"stop"}}
EOF
"#;

const ONE_TURN: &str = r#"cat <<'EOF'
{"type":"tool_execution_start","toolCallId":"t1","toolName":"bash"}
{"type":"message_end","message":{"role":"assistant","stopReason":"stop"}}
{"type":"turn_end","message":{"stopReason":"stop"}}
EOF
"#;

const MALFORMED: &str = r#"cat <<'EOF'
{"type":"tool_execution_start","toolCallId":"t1","toolName":"bash"}
this line is not a JSON event
EOF
"#;

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

fn sh() -> PathBuf {
    in_system(&["/usr/bin/sh", "/bin/sh"])
}

fn echo() -> PathBuf {
    in_system(&["/usr/bin/echo", "/bin/echo"])
}

fn fixture_root(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("sw-n1-05-pi-stream-{tag}-{}", std::process::id()))
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
            model_id: "fixture".to_string(),
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

fn write_script(work: &Path, name: &str, body: &str) {
    std::fs::write(work.join(name), body).expect("script записан");
}

fn copy_fixture(work: &Path) {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    std::fs::copy(&src, work.join("pi-fixture.sh"))
        .unwrap_or_else(|e| panic!("фикстура не скопирована из {}: {e}", src.display()));
}

fn exec(worker: &SandboxWorker<FixtureEngine>) -> Result<ExecOutcome, WorkerError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(worker.exec("workspace", &call()))
}

/// Прогон с кодом 0 обязан быть измерен: голый `ExecOutcome` не является результатом.
fn measured(worker: &SandboxWorker<FixtureEngine>) -> RunOutcome {
    let _ = exec(worker).expect("код 0 — процесс завершился, а не упал");
    worker.last_outcome().expect("прогон обязан быть измерен")
}

#[test]
fn the_fixture_stream_measures_one_step_two_turns_and_valid() {
    let root = fixture_root("valid");
    let worker = worker(&root, sh(), vec!["pi-fixture.sh".to_string()]);
    copy_fixture(&root.join("work"));

    let outcome = measured(&worker);
    assert_eq!(
        outcome.tool_steps, 1,
        "один tool_execution_start — один шаг"
    );
    assert_eq!(outcome.turns, 2, "два message_end — два хода");
    assert_eq!(
        outcome.stop_reason,
        StopReason::Stop,
        "turn_end назвал stop"
    );
    assert_eq!(
        outcome.verdict,
        Verdict::Valid,
        "шаг есть, ходов два, остановка чистая"
    );
    assert_eq!(outcome.total_input_tokens, 50, "usage прочитан из turn_end");
}

#[test]
fn a_stream_without_tool_execution_start_is_never_valid() {
    let root = fixture_root("no-tool");
    let worker = worker(&root, sh(), vec!["no-tool.sh".to_string()]);
    write_script(&root.join("work"), "no-tool.sh", NO_TOOL);

    let outcome = measured(&worker);
    assert_eq!(outcome.tool_steps, 0, "ни одного шага инструмента");
    assert_eq!(outcome.turns, 2, "ходы есть — их и не хватает");
    assert_ne!(
        outcome.verdict,
        Verdict::Valid,
        "говорила, но не действовала"
    );
    assert_eq!(outcome.verdict, Verdict::InvalidNoToolSteps);
}

#[test]
fn a_single_turn_is_below_the_threshold_and_never_valid() {
    let root = fixture_root("one-turn");
    let worker = worker(&root, sh(), vec!["one-turn.sh".to_string()]);
    write_script(&root.join("work"), "one-turn.sh", ONE_TURN);

    let outcome = measured(&worker);
    assert_eq!(outcome.tool_steps, 1, "шаг сделан");
    assert_eq!(outcome.turns, 1, "и ровно один ход");
    assert_ne!(
        outcome.verdict,
        Verdict::Valid,
        "turns = 1 не проходит порог «ходов больше одного»"
    );
    assert_eq!(outcome.verdict, Verdict::InvalidTooFewTurns);
}

#[test]
fn an_empty_stream_is_never_valid_and_that_is_correct() {
    let root = fixture_root("empty");
    let worker = worker(&root, echo(), Vec::new());

    let outcome = measured(&worker);
    assert_eq!(outcome.tool_steps, 0, "echo не печатает событий");
    assert_eq!(outcome.turns, 0, "и ходов у него нет");
    assert_ne!(
        outcome.verdict,
        Verdict::Valid,
        "пустой stdout — не успех: это дефект, который пакет и закрывает"
    );
    assert_eq!(outcome.verdict, Verdict::InvalidLoop);
}

#[test]
fn a_malformed_line_is_a_handled_refusal_and_not_silent_progress() {
    let root = fixture_root("malformed");
    let worker = worker(&root, sh(), vec!["malformed.sh".to_string()]);
    write_script(&root.join("work"), "malformed.sh", MALFORMED);

    // Процесс завершился кодом 0 — это smoke процесса, а не proof прогона. Поток
    // отвергнут: замер не состоялся, причина названа и видна, паники нет.
    let outcome = exec(&worker).expect("код 0 — процесс завершился, а не упал");
    assert!(!outcome.output.is_empty(), "текст процесса не потерян");
    assert_eq!(
        worker.last_outcome(),
        None,
        "отвергнутый поток не измеряется: замера нет, и это отличимо от пустого потока"
    );
    let refusal = worker.last_refusal().expect("отказ обязан быть назван");
    assert!(
        refusal.contains("not a Pi event"),
        "причина обязана называть виновника: {refusal:?}"
    );
}
