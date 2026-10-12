//! `stellwerk-cli` — минимальный вход в федерацию: `run <task.toml>`.
//!
//! N1-01: вертикальный срез. Читает task.toml, поднимает процесс ВНУТРИ bwrap с
//! очищенным окружением, проводит его через `runner::run_once`, пишет чек в SQLite
//! и печатает вердикт вместе с настоящим идентификатором чека.
//!
//! Движок захардкожен (`/bin/echo`): это smoke процесса, а не proof агентного
//! успеха. Настоящий движок — N1-02/N1-05.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use clap::{Parser, Subcommand};
use stellwerk_core::contract::{ModelBinding, PolicyLevel, RetentionClass, RiskClass};
use stellwerk_gateway::permission::ToolRisk;
use stellwerk_store::{EventStore, SqliteStore};
use stellwerk_task::m5::sandbox_bwrap::Sandbox;
use stellwerk_task::runner::{
    run_once, Call, CallAttempt, CallVerdict, ExecOutcome, Worker, WorkerError,
};
use stellwerk_task::taskfile::TaskFile;
use tokio::runtime::Runtime;

/// Системные каталоги, смонтированные в песочницу ТОЛЬКО НА ЧТЕНИЕ. bwrap стартует
/// с пустым корнем, поэтому без них `/bin/echo` нечем запустить. Шире
/// (`--ro-bind / /`) не берём: биндить весь хост-корень на чтение незачем, когда
/// программе нужен ровно этот набор:
///   * `/usr`  — сами бинарники (`usr/bin`) и разделяемые библиотеки (`usr/lib`), в т.ч. libc;
///   * `/bin`  — точка входа `/bin/echo` (`/bin` — симлинк на `usr/bin` при usr-merge);
///   * `/lib`, `/lib64` — динамический загрузчик и libc (`ld-linux-x86-64.so.2`, симлинки в `usr/lib`);
///   * `/etc`  — системная конфигурация, которую может запросить libc (NSS, локаль).
const SYSTEM_READ_ONLY: &[&str] = &["/usr", "/bin", "/lib", "/lib64", "/etc"];

/// Счётчик уникальности имён временных каталогов/файлов в пределах процесса: наносекунды
/// могут совпасть у двух быстрых прогонов, счётчик — нет.
static UNIQUE_SEQ: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Parser)]
#[command(name = "stellwerk-cli")]
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
}

type Exec<'a> = Pin<Box<dyn Future<Output = Result<ExecOutcome, WorkerError>> + Send + 'a>>;

/// Воркер smoke-прогона: запускает `/bin/echo <goal>` внутри bwrap.
struct EchoWorker {
    goal: String,
    window: Duration,
    /// Сколько раз воркер ФАКТИЧЕСКИ запускали, включая упавшие запуски.
    spawned: Arc<AtomicUsize>,
}

impl EchoWorker {
    fn new(goal: String, window: Duration) -> Self {
        Self {
            goal,
            window,
            spawned: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl Worker for EchoWorker {
    fn exec(&self, _workspace_ref: &str, _call: &Call) -> Exec<'_> {
        Box::pin(async move {
            self.spawned.fetch_add(1, Ordering::SeqCst);
            let work = unique_temp_path("work", "").map_err(WorkerError)?;
            // `tmp` — ОТДЕЛЬНЫЙ каталог: общий `/tmp` у двух песочниц — гонка за один файл.
            let tmp = unique_temp_path("tmp", "").map_err(WorkerError)?;
            std::fs::create_dir_all(&work).map_err(|e| WorkerError(e.to_string()))?;
            std::fs::create_dir_all(&tmp).map_err(|e| WorkerError(e.to_string()))?;

            let sandbox = Sandbox {
                work: work.clone(),
                tmp: tmp.clone(),
                read_only: SYSTEM_READ_ONLY.iter().map(PathBuf::from).collect(),
                window: self.window,
            };

            let mut command = sandbox
                .command(
                    &work,
                    Path::new("/bin/echo"),
                    &[self.goal.as_str()],
                    self.window,
                )
                .map_err(|e| WorkerError(format!("sandbox refused the run: {e}")))?;
            // N1: окружение порождённого процесса ОЧИЩЕНО. `command()` уже вернул
            // готовый `Command`, поэтому `m5/*` здесь не трогается.
            command.env_clear();

            let output = tokio::process::Command::from(command)
                .output()
                .await
                .map_err(|e| WorkerError(format!("process did not start: {e}")))?;

            // Провал запуска НЕЛЬЗЯ вернуть как успех: иначе `run_once` запишет
            // `Succeeded`, а CLI напечатает `verdict=Allow` при пустом выводе —
            // «misleading success» с неработающей изоляцией.
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(WorkerError(format!(
                    "process failed ({}): {}",
                    output.status,
                    stderr.trim()
                )));
            }

            let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
            let _ = std::fs::remove_dir_all(&work);
            let _ = std::fs::remove_dir_all(&tmp);
            Ok(ExecOutcome { output: stdout })
        })
    }

    fn spawned(&self) -> usize {
        self.spawned.load(Ordering::SeqCst)
    }
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
    }
}

fn run_task(path: &Path) -> Result<(), String> {
    let spec = TaskFile::load(path)
        .map_err(|e| format!("task file `{}`: {e}", path.display()))?
        .into_spec()
        .map_err(|e| format!("task file `{}`: {e}", path.display()))?;

    // Окно песочницы — из лимитов задачи, а не из константы.
    let window = Duration::from_secs(spec.limits.timeout_seconds);

    // База чека — уникальный файл на прогон: последовательные прогоны одной задачи не
    // делят `events.id` (счётчик чека в раннере начинается с нуля на каждый процесс).
    let db_path = unique_temp_path("receipts", "db")?;
    let store = SqliteStore::open(&db_path)
        .map_err(|e| format!("open store `{}`: {e}", db_path.display()))?;

    let worker = EchoWorker::new(spec.goal.clone(), window);

    let attempt = CallAttempt {
        task_id: spec.id.clone(),
        call: Call {
            tool: "echo".to_string(),
            module: "stellwerk-cli".to_string(),
            risk: ToolRisk::Classified(RiskClass::R0),
            data: spec.limits.data_class,
            model: ModelBinding {
                id: spec.model_ref.clone(),
                provider: "local".to_string(),
                model_id: "echo".to_string(),
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
    let receipt = runtime.block_on(receipt_id(&store, &attempt.task_id));

    match outcome {
        Ok(verdict) => {
            let id = receipt?;
            println!(
                "verdict={} receipt={} store={}",
                verdict_name(&verdict),
                id,
                db_path.display()
            );
            Ok(())
        }
        Err(err) => {
            match receipt {
                Ok(id) => eprintln!("receipt={} store={}", id, db_path.display()),
                Err(receipt_err) => eprintln!("receipt_error={receipt_err}"),
            }
            Err(format!("run failed: {}", err.0))
        }
    }
}

/// Идентификатор чека `run.call` для задачи. Чек пишет `run_once`; отсутствие чека —
/// состояние, а не пустая строка: прогон без следа неотличим от небывшего.
async fn receipt_id(store: &SqliteStore, task_id: &str) -> Result<String, String> {
    let events = store
        .list_by_task(task_id)
        .await
        .map_err(|e| format!("read receipts for `{task_id}`: {e}"))?;
    events
        .iter()
        .rev()
        .find(|event| event.event_type == "run.call")
        .map(|event| event.id.clone())
        .ok_or_else(|| format!("no run.call receipt written for `{task_id}`"))
}

fn verdict_name(verdict: &CallVerdict) -> String {
    match verdict {
        CallVerdict::Allow => "Allow".to_string(),
        CallVerdict::Ask => "Ask".to_string(),
        CallVerdict::Suspended => "Suspended".to_string(),
        CallVerdict::Deny { reason } => format!("Deny reason={reason}"),
    }
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
