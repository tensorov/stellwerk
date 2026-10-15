//! Библиотека `stellwerk-cli` — публичный интерфейс для тестов и плагинов.
//! BINARY-01: бинарник `stellwerk` лежит в main.rs; эта обёртка делает
//! `tui` видимым для интеграционных тестов. Бинарник не переиспользует
//! библиотеку — он свой модуль `mod tui` загружает, но оба указывают на
//! один файл (`src/tui.rs`), поэтому код дублируется только на уровне
//! `mod`/`pub mod` директивы. Это архитектурный выбор: `lib.rs` не тянет
//! `main.rs`-зависимости (clap, stellwerk-registry), а общий код —
//! `run_task`, `RunResult`, вспомогательные функции — живёт здесь.

pub mod plan;
pub mod tui;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use stellwerk_core::contract::{ModelBinding, PolicyLevel, RetentionClass, RiskClass};
use stellwerk_gateway::permission::ToolRisk;
use stellwerk_store::{EventStore, SqliteStore};
use stellwerk_task::evidence::{evidence_status, model_claim, tool_attestation, EvidenceLedger};
use stellwerk_task::ledger_sqlite::SqliteEvidenceLedger;
use stellwerk_task::m5::sandbox_bwrap::Sandbox;
use stellwerk_task::m5::FixtureEngine;
use stellwerk_task::orchestrator::{SandboxWorker, StaticRegistry, ToolEntry};
use stellwerk_task::runner::{run_once, Call, CallAttempt, CallVerdict, Verdict};
use stellwerk_task::taskfile::TaskFile;

/// Агент лежит рядом с task-файлом, как и у `run`: схема `TaskFile`
/// программу не несёт, её расположение задаёт каталог файла.
const CLI_AGENT_TOOLS: &[ToolEntry] = &[ToolEntry {
    module: "stellwerk-cli",
    tool: "fixture",
    class: RiskClass::R0,
}];

static UNIQUE_SEQ: AtomicU64 = AtomicU64::new(0);

/// Системные каталоги, смонтированные в песочницу ТОЛЬКО НА ЧТЕНИЕ.
const SYSTEM_READ_ONLY: &[&str] = &["/usr", "/bin", "/lib", "/lib64", "/etc"];

/// Результат прогона задачи, передаваемый от `run_task` к вызывающей стороне.
pub struct RunResult {
    pub verdict: String,
    pub receipt_id: String,
    pub store_path: PathBuf,
    pub gate: String,
    pub risk: String,
}

pub fn run_task(path: &Path) -> Result<RunResult, String> {
    let spec = TaskFile::load(path)
        .map_err(|e| format!("task file `{}`: {e}", path.display()))?
        .into_spec()
        .map_err(|e| format!("task file `{}`: {e}", path.display()))?;

    let fixture_src = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("pi-fixture.sh");
    if !fixture_src.is_file() {
        return Err(format!(
            "fixture `{}` not found next to the task file: N1 runs on the fixture agent",
            fixture_src.display()
        ));
    }

    let window = Duration::from_secs(spec.limits.timeout_seconds);
    let db_path = unique_temp_path("receipts", "db")?;
    let store = SqliteStore::open(&db_path)
        .map_err(|e| format!("open store `{}`: {e}", db_path.display()))?;

    let ledger_path = unique_temp_path("evidence", "db")?;
    let mut ledger = SqliteEvidenceLedger::open(&ledger_path)
        .map_err(|e| format!("open evidence ledger `{}`: {e}", ledger_path.display()))?;

    let work = unique_temp_path("work", "")?;
    let tmp = unique_temp_path("tmp", "")?;
    std::fs::create_dir_all(&work).map_err(|e| format!("create work dir: {e}"))?;
    std::fs::create_dir_all(&tmp).map_err(|e| format!("create tmp dir: {e}"))?;
    std::fs::copy(&fixture_src, work.join("pi-fixture.sh"))
        .map_err(|e| format!("copy fixture into sandbox: {e}"))?;

    let sandbox = Sandbox {
        work: work.clone(),
        tmp: tmp.clone(),
        read_only: SYSTEM_READ_ONLY.iter().map(PathBuf::from).collect(),
        window,
    };

    let sh = in_system(&["/usr/bin/sh", "/bin/sh"])?;
    let worker = SandboxWorker::new(
        FixtureEngine::new("/bin/true"),
        sandbox,
        sh.clone(),
        vec!["pi-fixture.sh".to_string()],
        window,
        Vec::new(),
    )
    .with_registry(StaticRegistry(CLI_AGENT_TOOLS));

    let attempt = CallAttempt {
        task_id: spec.id.clone(),
        call: Call {
            tool: "fixture".to_string(),
            module: "stellwerk-cli".to_string(),
            risk: ToolRisk::Classified(RiskClass::R0),
            data: spec.limits.data_class,
            model: ModelBinding {
                id: spec.model_ref.clone(),
                provider: "local".to_string(),
                model_id: "fixture".to_string(),
                data_class: spec.limits.data_class,
                retention: RetentionClass::FirstParty,
                context_window: 1_000,
                max_output: 1_000,
                budget_tokens_per_task: 1_000,
            },
            interactive: false,
        },
        caller: PolicyLevel::L2Contributor,
        workspace_ref: spec.workspace_ref.clone(),
    };

    let runtime = tokio::runtime::Runtime::new().map_err(|e| format!("tokio runtime: {e}"))?;
    let outcome = runtime.block_on(run_once(&attempt, &worker, &store));

    let receipt = runtime.block_on(receipt(&store, &attempt.task_id));

    match outcome {
        Ok(verdict) => {
            let (id, digest, ts, risk) = receipt?;
            let measured = worker.last_outcome().ok_or_else(|| {
                format!(
                    "run produced no measured outcome: {}",
                    worker
                        .last_refusal()
                        .unwrap_or_else(|| "no refusal recorded".to_string())
                )
            })?;
            if measured.verdict != Verdict::Valid {
                return Err(format!(
                    "run outcome is not Valid: {:?} (stop={:?}, steps={}, turns={})",
                    measured.verdict, measured.stop_reason, measured.tool_steps, measured.turns
                ));
            }

            let command = format!("{} pi-fixture.sh", sh.display());
            let chain = vec![
                model_claim(
                    &attempt.task_id,
                    &format!("{}-claim", attempt.task_id),
                    &digest,
                    &ts,
                ),
                tool_attestation(
                    &attempt.task_id,
                    &format!("{}-attest", attempt.task_id),
                    &command,
                    &digest,
                    &ts,
                ),
            ];
            for entry in &chain {
                ledger
                    .append(entry)
                    .map_err(|e| format!("evidence append: {e}"))?;
            }
            let status = evidence_status(&chain).label();
            Ok(RunResult {
                verdict: verdict_name(&verdict),
                receipt_id: id,
                store_path: db_path,
                gate: status.to_string(),
                risk,
            })
        }
        Err(err) => {
            match receipt {
                Ok((id, _, _, _)) => eprintln!("receipt={} store={}", id, db_path.display()),
                Err(receipt_err) => eprintln!("receipt_error={receipt_err}"),
            }
            Err(format!("run failed: {}", err.0))
        }
    }
}

pub(crate) async fn receipt(
    store: &SqliteStore,
    task_id: &str,
) -> Result<(String, String, String, String), String> {
    let events = store
        .list_by_task(task_id)
        .await
        .map_err(|e| format!("read receipts for `{task_id}`: {e}"))?;
    let event = events
        .iter()
        .rev()
        .find(|event| event.event_type == "run.call")
        .ok_or_else(|| format!("no run.call receipt written for `{task_id}`"))?;
    let digest = event
        .payload
        .get("output_sha256")
        .cloned()
        .ok_or_else(|| format!("run.call receipt for `{task_id}` has no output_sha256"))?;
    let risk = event
        .payload
        .get("risk")
        .cloned()
        .unwrap_or_else(|| "Unclassified".to_string());
    Ok((event.id.clone(), digest, event.ts.clone(), risk))
}

pub(crate) fn verdict_name(verdict: &CallVerdict) -> String {
    match verdict {
        CallVerdict::Allow => "Allow".to_string(),
        CallVerdict::Ask => "Ask".to_string(),
        CallVerdict::Suspended => "Suspended".to_string(),
        CallVerdict::Deny { reason } => format!("Deny reason={reason}"),
    }
}

pub(crate) fn in_system(names: &[&str]) -> Result<PathBuf, String> {
    names
        .iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
        .ok_or_else(|| format!("none of {names:?} exists on this system"))
}

pub(crate) fn unique_temp_path(kind: &str, extension: &str) -> Result<PathBuf, String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("clock before unix epoch: {e}"))?
        .as_nanos();
    let seq = UNIQUE_SEQ.fetch_add(1, Ordering::Relaxed);
    let stem = format!("stellwerk-n1-{kind}-{}-{nanos}-{seq}", std::process::id());
    let name = if extension.is_empty() {
        stem
    } else {
        format!("{stem}.{extension}")
    };
    Ok(std::env::temp_dir().join(name))
}
