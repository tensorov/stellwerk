//! Библиотека `stellwerk-cli` — публичный интерфейс для тестов и плагинов.
//! BINARY-01: бинарник `stellwerk` лежит в main.rs; эта обёртка делает
//! `tui` видимым для интеграционных тестов. Бинарник не переиспользует
//! библиотеку — он свой модуль `mod tui` загружает, но оба указывают на
//! один файл (`src/tui.rs`), поэтому код дублируется только на уровне
//! `mod`/`pub mod` директивы. Это архитектурный выбор: `lib.rs` не тянет
//! `main.rs`-зависимости (clap, stellwerk-registry), а общий код —
//! `run_task`, `RunResult`, вспомогательные функции — живёт здесь.

pub mod build_runner;
pub mod host_model;
pub mod plan;
pub mod planner_host;
pub mod tui;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};
use stellwerk_core::contract::{
    CommitSha, ModelBinding, PolicyDigest, PolicyLevel, RecipeDigest, RetentionClass, RiskClass,
};
use stellwerk_gateway::permission::ToolRisk;
use stellwerk_store::{EventStore, SqliteStore};
use stellwerk_task::evidence::{
    evidence_status, model_claim, tool_attestation, EvidenceEntry, EvidenceLedger,
};
use stellwerk_task::ledger_sqlite::SqliteEvidenceLedger;
use stellwerk_task::m5::sandbox_bwrap::Sandbox;
use stellwerk_task::m5::FixtureEngine;
use stellwerk_task::orchestrator::{pi_transport_args, SandboxWorker, StaticRegistry, ToolEntry};
use stellwerk_task::runner::{run_once, Call, CallAttempt, CallVerdict, Verdict, Worker};
use stellwerk_task::taskfile::TaskFile;

/// Реестр инструментов CLI-прогона. Движок — Pi по профилю `pi`; класс риска
/// объявлен здесь, а не взят из production-реестра оркестратора.
const CLI_AGENT_TOOLS: &[ToolEntry] = &[ToolEntry {
    module: "stellwerk-cli",
    tool: "pi",
    class: RiskClass::R0,
}];

const PI_BIN: &str = "PI_BIN";
const PI_EXTENSION: &str = "STELLWERK_PI_EXTENSION";

/// Найти Pi: `PI_BIN`, затем `$HOME/.pi/agent/bin/pi` — те же правила, что у
/// `plan.rs::pi_program`. Отсутствие файла — названный `MissingEngine`, не фикстура.
pub(crate) fn pi_program() -> Result<PathBuf, String> {
    let candidate = match std::env::var_os(PI_BIN) {
        Some(path) => PathBuf::from(path),
        None => {
            let home = std::env::var_os("HOME").ok_or_else(|| {
                format!("MissingEngine: neither {PI_BIN} nor HOME is set, so Pi cannot be located")
            })?;
            PathBuf::from(home).join(".pi/agent/bin/pi")
        }
    };
    if candidate.is_file() {
        Ok(candidate)
    } else {
        Err(format!(
            "MissingEngine: Pi program `{}` does not exist (set {PI_BIN} or install Pi under $HOME/.pi/agent/bin/pi)",
            candidate.display()
        ))
    }
}

/// Путь к транспорту Stellwerk. Без расширения Pi не соединён с каналом модели,
/// поэтому отсутствие — названный отказ, а не прогон по умолчанию.
fn pi_extension() -> Result<PathBuf, String> {
    let path = std::env::var_os(PI_EXTENSION)
        .map(PathBuf::from)
        .ok_or_else(|| {
            format!(
                "MissingPiExtension: {PI_EXTENSION} is not set (path to stellwerk_transport.ts)"
            )
        })?;
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!(
            "MissingPiExtension: `{}` is not a file",
            path.display()
        ))
    }
}

static UNIQUE_SEQ: AtomicU64 = AtomicU64::new(0);

/// Системные каталоги, смонтированные в песочницу ТОЛЬКО НА ЧТЕНИЕ.
const SYSTEM_READ_ONLY: &[&str] = &["/usr", "/bin", "/lib", "/lib64", "/etc"];

/// Результат прогона задачи, передаваемый от `run_task` к вызывающей стороне.
pub struct RunResult {
    pub verdict: String,
    pub receipt_id: String,
    pub store_path: PathBuf,
    pub ledger_path: PathBuf,
    pub gate: String,
    pub risk: String,
}

fn missing_binding(detail: impl std::fmt::Display) -> String {
    format!("MissingAttestationBinding: commit_sha {detail}")
}

fn sha256_hex(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

/// Привязки production-удостоверения: рецепт исполнения, политика и коммит.
/// Все три обязательны: удостоверение без любой из них не привязано, а лишь
/// перечисляет два из трёх.
struct AttestationBindings {
    recipe_digest: RecipeDigest,
    policy_digest: PolicyDigest,
    commit_sha: CommitSha,
}

impl AttestationBindings {
    fn resolve(program: &Path, args: &[String]) -> Result<Self, String> {
        let recipe = std::iter::once(program.display().to_string())
            .chain(args.iter().cloned())
            .collect::<Vec<_>>()
            .join("\0");
        Ok(Self {
            recipe_digest: RecipeDigest(sha256_hex(&recipe)),
            policy_digest: PolicyDigest(sha256_hex("stellwerk-cli|pi|R0|L2Contributor")),
            commit_sha: CommitSha(read_commit_sha(&git_root()?)?),
        })
    }
}

fn is_commit_sha(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Ближайший предок cwd, содержащий `.git`: cwd может лежать в подкаталоге
/// рабочего дерева, и требовать `.git` именно в нём значило бы отказать
/// законному прогону.
fn git_root() -> Result<PathBuf, String> {
    let cwd = std::env::current_dir()
        .map_err(|e| missing_binding(format!("needs a working directory: {e}")))?;
    cwd.ancestors()
        .find(|dir| dir.join(".git").exists())
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            missing_binding(format!(
                "has no `.git` in any ancestor of `{}`",
                cwd.display()
            ))
        })
}

/// SHA коммита рабочего дерева. `.git` может быть каталогом, файлом-указателем
/// (worktree) или отсутствовать; `commondir` переносит ссылки в общий каталог.
/// Любая неоднозначность — названный отказ: выдуманный SHA привязал бы
/// удостоверение к чужому коммиту.
fn read_commit_sha(root: &Path) -> Result<String, String> {
    let dot_git = root.join(".git");
    let gitdir = if dot_git.is_dir() {
        dot_git
    } else if dot_git.is_file() {
        let pointer = std::fs::read_to_string(&dot_git)
            .map_err(|e| missing_binding(format!("`{}` is unreadable: {e}", dot_git.display())))?;
        let target = pointer
            .lines()
            .next()
            .and_then(|line| line.strip_prefix("gitdir: "))
            .ok_or_else(|| {
                missing_binding(format!(
                    "`{}` is not a `gitdir: <path>` pointer",
                    dot_git.display()
                ))
            })?
            .trim();
        let path = PathBuf::from(target);
        if path.is_absolute() {
            path
        } else {
            root.join(path)
        }
    } else {
        return Err(missing_binding(format!(
            "`{}` is neither a directory nor a file",
            dot_git.display()
        )));
    };

    let head_path = gitdir.join("HEAD");
    let head = std::fs::read_to_string(&head_path).map_err(|e| {
        missing_binding(format!("HEAD `{}` is unreadable: {e}", head_path.display()))
    })?;
    let head = head.trim();
    let Some(refname) = head.strip_prefix("ref: ") else {
        if is_commit_sha(head) {
            return Ok(head.to_owned());
        }
        return Err(missing_binding(format!(
            "HEAD `{}` is neither a ref nor a commit",
            head_path.display()
        )));
    };
    let refname = refname.trim();

    let common = gitdir.join("commondir");
    let refroot = if common.exists() {
        let text = std::fs::read_to_string(&common).map_err(|e| {
            missing_binding(format!(
                "commondir `{}` is unreadable: {e}",
                common.display()
            ))
        })?;
        let line = text.lines().next().unwrap_or("").trim();
        if line.is_empty() {
            return Err(missing_binding(format!(
                "commondir `{}` is empty",
                common.display()
            )));
        }
        let path = PathBuf::from(line);
        if path.is_absolute() {
            path
        } else {
            gitdir.join(path)
        }
    } else {
        gitdir.clone()
    };

    let ref_path = refroot.join(refname);
    if let Ok(value) = std::fs::read_to_string(&ref_path) {
        let value = value.trim();
        if is_commit_sha(value) {
            return Ok(value.to_owned());
        }
        return Err(missing_binding(format!(
            "ref `{}` holds `{value}`, not a 40-hex commit",
            ref_path.display()
        )));
    }

    let packed_path = refroot.join("packed-refs");
    let packed = std::fs::read_to_string(&packed_path).map_err(|e| {
        missing_binding(format!(
            "ref `{refname}` is absent from `{}` and `{}` is unreadable: {e}",
            ref_path.display(),
            packed_path.display()
        ))
    })?;
    for line in packed.lines() {
        if line.starts_with('#') || line.starts_with('^') {
            continue;
        }
        if let Some((sha, name)) = line.split_once(' ') {
            if name == refname && is_commit_sha(sha) {
                return Ok(sha.to_owned());
            }
        }
    }
    Err(missing_binding(format!(
        "ref `{refname}` is absent from `{}` and `{}`",
        ref_path.display(),
        packed_path.display()
    )))
}

struct AttestationInput<'a> {
    task_id: &'a str,
    id: &'a str,
    command: &'a str,
    result_hash: &'a str,
    ts: &'a str,
}

/// `ToolAttestation` с привязками. Конструктор `tool_attestation` остаётся
/// единственным местом формы записи, поэтому EvidenceEntry здесь не собирается
/// заново — только дополняется обязательными полями.
fn bound_attestation(input: AttestationInput<'_>, bindings: &AttestationBindings) -> EvidenceEntry {
    let mut entry = tool_attestation(
        input.task_id,
        input.id,
        input.command,
        input.result_hash,
        input.ts,
    );
    entry.recipe_digest = Some(bindings.recipe_digest.clone());
    entry.policy_digest = Some(bindings.policy_digest.clone());
    entry.commit_sha = Some(bindings.commit_sha.clone());
    entry
}

/// Единственный путь исполнения для Run и Plan-Build.
///
/// Решение выносит `run_once`, то есть `authorize` по доверенному реестру воркера;
/// чек пишется на каждый вызов, включая отказ. Отсутствие чека поэтому означает
/// «авторизации не было», а не «прошло без замечаний».
pub(crate) async fn execute(
    attempt: &CallAttempt,
    worker: &dyn Worker,
    store: &SqliteStore,
) -> Result<CallVerdict, String> {
    run_once(attempt, worker, store).await.map_err(|e| e.0)
}

pub fn run_task(path: &Path) -> Result<RunResult, String> {
    let spec = TaskFile::load(path)
        .map_err(|e| format!("task file `{}`: {e}", path.display()))?
        .into_spec()
        .map_err(|e| format!("task file `{}`: {e}", path.display()))?;

    let pi = pi_program()?;
    let extension = pi_extension()?;

    let window = Duration::from_secs(spec.limits.timeout_seconds);
    let db_path = unique_temp_path("receipts", "db")?;
    let store = SqliteStore::open(&db_path)
        .map_err(|e| format!("open store `{}`: {e}", db_path.display()))?;

    let ledger_path = unique_temp_path("evidence", "db")?;
    let mut ledger = SqliteEvidenceLedger::open(&ledger_path)
        .map_err(|e| format!("open evidence ledger `{}`: {e}", ledger_path.display()))?;

    let args = pi_transport_args(&extension, &[]);
    let bindings = AttestationBindings::resolve(&pi, &args).inspect_err(|_| {
        eprintln!("ledger={}", ledger_path.display());
    })?;

    let work = unique_temp_path("work", "")?;
    let tmp = unique_temp_path("tmp", "")?;
    let home = work.join("home");
    std::fs::create_dir_all(&work).map_err(|e| format!("create work dir: {e}"))?;
    std::fs::create_dir_all(&tmp).map_err(|e| format!("create tmp dir: {e}"))?;
    std::fs::create_dir_all(&home).map_err(|e| format!("create agent home: {e}"))?;

    let mut read_only: Vec<PathBuf> = SYSTEM_READ_ONLY.iter().map(PathBuf::from).collect();
    if let Some(bin) = pi.parent().filter(|p| !p.as_os_str().is_empty()) {
        read_only.push(bin.to_path_buf());
        if let Some(install) = bin.parent().map(|agent| agent.join("install")) {
            if install.is_dir() {
                read_only.push(install);
            }
        }
    }
    if let Some(dir) = extension.parent().filter(|p| !p.as_os_str().is_empty()) {
        read_only.push(dir.to_path_buf());
    }

    let sandbox = Sandbox {
        work: work.clone(),
        tmp: tmp.clone(),
        read_only,
        window,
    };

    let args = pi_transport_args(&extension, &[]);
    let worker = SandboxWorker::new(
        FixtureEngine::new(pi.clone()),
        sandbox,
        pi.clone(),
        args.clone(),
        window,
        vec![
            ("HOME".to_string(), home.display().to_string()),
            ("PATH".to_string(), "/usr/bin:/bin".to_string()),
            ("TMPDIR".to_string(), tmp.display().to_string()),
        ],
    )
    .with_registry(StaticRegistry(CLI_AGENT_TOOLS));

    let attempt = CallAttempt {
        task_id: spec.id.clone(),
        call: Call {
            tool: "pi".to_string(),
            module: "stellwerk-cli".to_string(),
            risk: ToolRisk::Classified(RiskClass::R0),
            data: spec.limits.data_class,
            model: ModelBinding {
                id: spec.model_ref.clone(),
                provider: "local".to_string(),
                model_id: "pi".to_string(),
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
    let outcome = runtime.block_on(execute(&attempt, &worker, &store));

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

            let command = format!("{} {}", pi.display(), args.join(" "));
            let attest_id = format!("{}-attest", attempt.task_id);
            let claim_id = format!("{}-claim", attempt.task_id);
            let chain = vec![
                model_claim(&attempt.task_id, &claim_id, &digest, &ts),
                bound_attestation(
                    AttestationInput {
                        task_id: &attempt.task_id,
                        id: &attest_id,
                        command: &command,
                        result_hash: &digest,
                        ts: &ts,
                    },
                    &bindings,
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
                ledger_path,
                gate: status.to_string(),
                risk,
            })
        }
        Err(err) => {
            match receipt {
                Ok((id, _, _, _)) => eprintln!("receipt={} store={}", id, db_path.display()),
                Err(receipt_err) => eprintln!("receipt_error={receipt_err}"),
            }
            Err(format!("run failed: {}", err))
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
