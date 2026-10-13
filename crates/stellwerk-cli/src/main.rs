//! `stellwerk-cli` — единый вход в федерацию: `run <task.toml>` и `module ...`.
//!
//! N1-10: `run` собирает всю цепочку N1 на fixture-агенте: вердикт → receipt →
//! Pi-`RunOutcome` → evidence-цепочка → статус «ждёт человека». `module`
//! делегирует в `stellwerk_registry` — логика команд не дублируется.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use clap::{Parser, Subcommand};
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
use tokio::runtime::Runtime;

/// Системные каталоги, смонтированные в песочницу ТОЛЬКО НА ЧТЕНИЕ. bwrap стартует
/// с пустым корнем, поэтому без них `sh` нечем запустить. Шире (`--ro-bind / /`)
/// не берём: биндить весь хост-корень на чтение незачем, когда программе нужен
/// ровно этот набор:
///   * `/usr`  — сами бинарники (`usr/bin`) и разделяемые библиотеки (`usr/lib`), в т.ч. libc;
///   * `/bin`  — точка входа `/bin/sh` (`/bin` — симлинк на `usr/bin` при usr-merge);
///   * `/lib`, `/lib64` — динамический загрузчик и libc (`ld-linux-x86-64.so.2`, симлинки в `usr/lib`);
///   * `/etc`  — системная конфигурация, которую может запросить libc (NSS, локаль).
const SYSTEM_READ_ONLY: &[&str] = &["/usr", "/bin", "/lib", "/lib64", "/etc"];

/// Реестр риска АГЕНТА CLI — фикстуры `pi-fixture.sh`, которую запускает `run`.
///
/// Это отдельный от `stellwerk_task::orchestrator::TOOL_REGISTRY` реестр: фикстура
/// не является production-инструментом, и её класс нельзя выдавать из общего
/// реестра — иначе код, называющий своего агента «fixture» и назначающий ему R0,
/// разрешал бы вызов сам себе (дефект N1). R0 честен для этой операции: агент
/// локально печатает фиксированный поток, побочных эффектов нет.
static CLI_AGENT_TOOLS: &[ToolEntry] = &[ToolEntry {
    module: "stellwerk-cli",
    tool: "fixture",
    class: RiskClass::R0,
}];

/// Счётчик уникальности имён временных каталогов/файлов в пределах процесса: наносекунды
/// могут совпасть у двух быстрых прогонов, счётчик — нет.
static UNIQUE_SEQ: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Parser)]
#[command(name = "stellwerk")]
struct Cli {
    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Run a task from a task.toml file
    Run {
        /// Path to the task.toml file
        path: PathBuf,
    },
    /// Manage federation modules (delegated to stellwerk-registry)
    Module {
        /// Arguments forwarded to the registry CLI
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<std::ffi::OsString>,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.cmd {
        Commands::Run { path } => {
            if let Err(err) = run_task(&path) {
                eprintln!("error: {err}");
                std::process::exit(1);
            }
        }
        Commands::Module { args } => {
            let mut argv = vec![
                std::ffi::OsString::from("stellwerk"),
                std::ffi::OsString::from("module"),
            ];
            argv.extend(args);
            std::process::exit(stellwerk_registry::cli::run(argv));
        }
    }
}

fn run_task(path: &Path) -> Result<(), String> {
    let spec = TaskFile::load(path)
        .map_err(|e| format!("task file `{}`: {e}", path.display()))?
        .into_spec()
        .map_err(|e| format!("task file `{}`: {e}", path.display()))?;

    // Фикстура лежит РЯДОМ с файлом задачи: `pi-smoke.toml` указывает на
    // `pi-fixture.sh` своим расположением, а не полем в TOML (схема TaskFile
    // программу не несёт).
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

    // Окно песочницы — из лимитов задачи; бюджет исполнения обязан помещаться в окно.
    let window = Duration::from_secs(spec.limits.timeout_seconds);
    let budget = window;

    // База чека — уникальный файл на прогон: последовательные прогоны одной задачи не
    // делят `events.id` (счётчик чека в раннере начинается с нуля на каждый процесс).
    let db_path = unique_temp_path("receipts", "db")?;
    let store = SqliteStore::open(&db_path)
        .map_err(|e| format!("open store `{}`: {e}", db_path.display()))?;

    // Цепочка доказательств — ОТДЕЛЬНАЯ база: отказ её хранилища не имеет права
    // ронять event-лог (ledger_sqlite.rs).
    let ledger_path = unique_temp_path("evidence", "db")?;
    let mut ledger = SqliteEvidenceLedger::open(&ledger_path)
        .map_err(|e| format!("open evidence ledger `{}`: {e}", ledger_path.display()))?;

    // Рабочий каталог песочницы: фикстура копируется ВНУТРЬ, а не запускается с хоста.
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
        budget,
        Vec::new(),
    )
    .with_registry(StaticRegistry(CLI_AGENT_TOOLS));

    let attempt = CallAttempt {
        task_id: spec.id.clone(),
        call: Call {
            tool: "fixture".to_string(),
            module: "stellwerk-cli".to_string(),
            // Самодекларация НЕ авторитетна: эффективный класс `fixture` берётся из
            // реестра `CLI_AGENT_TOOLS` (выше), где он записан R0 — локальное
            // чтение фикстуры без побочных эффектов. Значение здесь не влияет на
            // вердикт и оставлено лишь как заполнитель поля `Call`.
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

    let runtime = Runtime::new().map_err(|e| format!("tokio runtime: {e}"))?;
    let outcome = runtime.block_on(run_once(&attempt, &worker, &store));

    // Чек читается ОБРАТНО из хранилища: печатать `task_id` под видом чека — подделка.
    let receipt = runtime.block_on(receipt(&store, &attempt.task_id));

    match outcome {
        Ok(verdict) => {
            let (id, digest, ts) = receipt?;
            // Pi-RunOutcome: замер обязан состояться, и вердикт прогона — Valid.
            // Голый `ExecOutcome` результатом не является (N1-05).
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
            // Evidence-цепочка: утверждение модели + удостоверение фактического
            // запуска. Обе записи ссылаются на дайджест вывода из чека и несут
            // метку времени самого прогона (чека), а не момент записи.
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
            // Статус по цепочке: удостоверение есть, решения человека нет — «ждёт
            // человека». `Done`/`gate PASS` не выводятся (решение 5).
            let status = evidence_status(&chain).label();
            println!(
                "verdict={} receipt={} store={} gate={}",
                verdict_name(&verdict),
                id,
                db_path.display(),
                status
            );
            Ok(())
        }
        Err(err) => {
            match receipt {
                Ok((id, _, _)) => eprintln!("receipt={} store={}", id, db_path.display()),
                Err(receipt_err) => eprintln!("receipt_error={receipt_err}"),
            }
            Err(format!("run failed: {}", err.0))
        }
    }
}

/// Чек `run.call` для задачи: id, дайджест вывода и метка времени, прочитанные
/// обратно из SQLite. Отсутствие чека — состояние, а не пустая строка: прогон
/// без следа неотличим от небывшего.
async fn receipt(store: &SqliteStore, task_id: &str) -> Result<(String, String, String), String> {
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
    Ok((event.id.clone(), digest, event.ts.clone()))
}

fn verdict_name(verdict: &CallVerdict) -> String {
    match verdict {
        CallVerdict::Allow => "Allow".to_string(),
        CallVerdict::Ask => "Ask".to_string(),
        CallVerdict::Suspended => "Suspended".to_string(),
        CallVerdict::Deny { reason } => format!("Deny reason={reason}"),
    }
}

/// Первый существующий путь из списка: `/usr/bin/sh` при usr-merge, иначе `/bin/sh`.
fn in_system(names: &[&str]) -> Result<PathBuf, String> {
    names
        .iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
        .ok_or_else(|| format!("none of {names:?} exists on this system"))
}

/// Уникальный путь под `temp_dir` с расширением (`""` — без расширения).
fn unique_temp_path(kind: &str, extension: &str) -> Result<PathBuf, String> {
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
