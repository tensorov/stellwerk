//! Вертикальный тест N1: полная цепочка run → verdict → receipt → Pi-RunOutcome →
//! evidence → «ждёт человека» через публичный API, и честные заглушки cancel/logs
//! через бинарник.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use stellwerk_core::contract::{DataClass, ModelBinding, PolicyLevel, RetentionClass, RiskClass};
use stellwerk_gateway::permission::ToolRisk;
use stellwerk_store::{EventStore, SqliteStore};
use stellwerk_task::evidence::{evidence_status, model_claim, tool_attestation, EvidenceLedger};
use stellwerk_task::ledger_sqlite::SqliteEvidenceLedger;
use stellwerk_task::m5::sandbox_bwrap::Sandbox;
use stellwerk_task::m5::FixtureEngine;
use stellwerk_task::orchestrator::{SandboxWorker, StaticRegistry, ToolEntry};
use stellwerk_task::runner::{run_once, Call, CallAttempt, CallVerdict, Verdict};

/// Системные каталоги на чтение — тот же набор, что у `stellwerk-cli` (N1-10):
/// bwrap стартует с пустым корнем, и без них `sh` нечем запустить.
const SYSTEM_READ_ONLY: &[&str] = &["/usr", "/bin", "/lib", "/lib64", "/etc"];

/// Доверенный реестр ЭТОГО теста: production-реестр намеренно не содержит
/// фикстурный агент CLI (свой класс он объявляет в реестре, который CLI собирает
/// у себя). Класс обязан приходить через `.with_registry(...)` — тот же шов, что
/// и production: без реестра `fixture`/`stellwerk-cli` был бы `Unclassified` и
/// отвергнут до вопроса (закон 6).
static N1_TOOL_REGISTRY: &[ToolEntry] = &[ToolEntry {
    module: "stellwerk-cli",
    tool: "fixture",
    class: RiskClass::R0,
}];

static UNIQUE_SEQ: AtomicU64 = AtomicU64::new(0);

fn unique_temp_path(kind: &str, extension: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before unix epoch")
        .as_nanos();
    let seq = UNIQUE_SEQ.fetch_add(1, Ordering::Relaxed);
    let stem = format!(
        "stellwerk-n1-test-{kind}-{}-{nanos}-{seq}",
        std::process::id()
    );
    let name = if extension.is_empty() {
        stem
    } else {
        format!("{stem}.{extension}")
    };
    std::env::temp_dir().join(name)
}

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../stellwerk-cli/examples/pi-fixture.sh")
}

fn sh_path() -> PathBuf {
    ["/usr/bin/sh", "/bin/sh"]
        .iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
        .expect("sh exists")
}

#[tokio::test]
async fn the_full_chain_ends_in_awaiting_human_validation() {
    let fixture = fixture_path();
    assert!(fixture.is_file(), "fixture missing: {}", fixture.display());

    let db_path = unique_temp_path("receipts", "db");
    let store = SqliteStore::open(&db_path).expect("open store");
    let ledger_path = unique_temp_path("evidence", "db");
    let mut ledger = SqliteEvidenceLedger::open(&ledger_path).expect("open ledger");

    let work = unique_temp_path("work", "");
    let tmp = unique_temp_path("tmp", "");
    std::fs::create_dir_all(&work).expect("create work");
    std::fs::create_dir_all(&tmp).expect("create tmp");
    std::fs::copy(&fixture, work.join("pi-fixture.sh")).expect("copy fixture");

    let sandbox = Sandbox {
        work: work.clone(),
        tmp: tmp.clone(),
        read_only: SYSTEM_READ_ONLY.iter().map(PathBuf::from).collect(),
        window: Duration::from_secs(30),
    };
    let sh = sh_path();
    let worker = SandboxWorker::new(
        FixtureEngine::new("/bin/true"),
        sandbox,
        sh.clone(),
        vec!["pi-fixture.sh".to_string()],
        Duration::from_secs(30),
        Vec::new(),
    )
    .with_registry(StaticRegistry(N1_TOOL_REGISTRY));

    let task_id = "n1-vertical".to_string();
    let attempt = CallAttempt {
        task_id: task_id.clone(),
        call: Call {
            tool: "fixture".to_string(),
            module: "stellwerk-cli".to_string(),
            risk: ToolRisk::Classified(RiskClass::R0),
            data: DataClass::Internal,
            model: ModelBinding {
                id: "m-n1".to_string(),
                provider: "local".to_string(),
                model_id: "fixture".to_string(),
                data_class: DataClass::Internal,
                retention: RetentionClass::FirstParty,
                context_window: 1_000,
                max_output: 1_000,
                budget_tokens_per_task: 1_000,
            },
            interactive: false,
        },
        caller: PolicyLevel::L2Contributor,
        workspace_ref: "ws-n1".to_string(),
    };

    let verdict = run_once(&attempt, &worker, &store)
        .await
        .expect("run_once ok");
    assert_eq!(verdict, CallVerdict::Allow);

    let measured = worker.last_outcome().expect("measured outcome");
    assert_eq!(measured.verdict, Verdict::Valid);

    let events = store.list_by_task(&task_id).await.expect("list events");
    let receipt = events
        .iter()
        .rev()
        .find(|event| event.event_type == "run.call")
        .expect("run.call receipt");
    assert_ne!(receipt.id, task_id, "receipt id must not be the task id");
    assert_eq!(
        receipt.payload.get("status").map(String::as_str),
        Some("Succeeded")
    );
    let digest = receipt
        .payload
        .get("output_sha256")
        .expect("receipt has output_sha256");

    let chain = vec![
        model_claim(&task_id, &format!("{task_id}-claim"), digest, &receipt.ts),
        tool_attestation(
            &task_id,
            &format!("{task_id}-attest"),
            &format!("{} pi-fixture.sh", sh.display()),
            digest,
            &receipt.ts,
        ),
    ];
    for entry in &chain {
        ledger.append(entry).expect("append evidence");
    }
    let entries = ledger.entries(&task_id);
    assert_eq!(entries.len(), 2, "claim + attestation");

    let status = evidence_status(&chain);
    assert_eq!(status.label(), "ждёт человека");
    assert_ne!(status.label(), "Done");

    let unknown = CallAttempt {
        task_id: "n1-vertical-unknown".to_string(),
        call: Call {
            tool: "unregistered-tool".to_string(),
            ..attempt.call.clone()
        },
        caller: PolicyLevel::L2Contributor,
        workspace_ref: "ws-n1".to_string(),
    };
    let refused = run_once(&unknown, &worker, &store)
        .await
        .expect("run_once ok");
    assert!(
        matches!(refused, CallVerdict::Deny { .. }),
        "тот же шов реестра обязан отвергнуть неклассифицированный инструмент, got {refused:?}"
    );
}

#[test]
fn cancel_and_logs_name_the_n2_capability_and_exit_nonzero() {
    let bin = env!("CARGO_BIN_EXE_stellwerk-task");
    for command in ["cancel", "logs"] {
        let db = unique_temp_path("stub-db", "db");
        let output = Command::new(bin)
            .arg("task")
            .arg(command)
            .arg("t-1")
            .env("STELLWERK_DB", &db)
            .output()
            .expect("task binary should spawn");
        assert_eq!(
            output.status.code(),
            Some(1),
            "{command} must exit non-zero"
        );
        assert!(
            output.stdout.is_empty(),
            "{command} must not print to stdout"
        );
        let expected = format!("not implemented: {command} (N2+)");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(&expected),
            "{command}: missing `{expected}` in stderr: {stderr}"
        );
        assert!(
            !stderr.contains("Done"),
            "{command}: must not claim Done: {stderr}"
        );
        let _ = std::fs::remove_file(&db);
    }
}
