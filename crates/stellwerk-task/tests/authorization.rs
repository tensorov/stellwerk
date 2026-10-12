use std::path::{Path, PathBuf};
use std::time::Duration;

use stellwerk_core::contract::{DataClass, ModelBinding, PolicyLevel, RetentionClass, RiskClass};
use stellwerk_gateway::permission::ToolRisk;
use stellwerk_store::{EventStore, SqliteStore};
use stellwerk_task::m5::sandbox_bwrap::Sandbox;
use stellwerk_task::m5::secrets::{agent_argv, AgentEnv, SecretError, Token};
use stellwerk_task::m5::FixtureEngine;
use stellwerk_task::orchestrator::SandboxWorker;
use stellwerk_task::runner::{run_once, Call, CallAttempt, CallVerdict, Worker};

const WINDOW: Duration = Duration::from_secs(60);
const BUDGET: Duration = Duration::from_secs(30);
const TOKEN_NAME: &str = "GITHUB_TOKEN";
const TOKEN_VALUE: &str = "ghp-n1-09-never-in-the-agent-reach";
const R0: ToolRisk = ToolRisk::Classified(RiskClass::R0);
const R5: ToolRisk = ToolRisk::Classified(RiskClass::R5);

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
        .unwrap()
}
fn fixture_root(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("sw-n1-09-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    root
}
fn sandbox(root: &Path) -> Sandbox {
    let work = root.join("work");
    let tmp = root.join("tmp");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::create_dir_all(&tmp).unwrap();
    Sandbox {
        work,
        tmp,
        read_only: system_dirs(),
        window: WINDOW,
    }
}
fn marker(root: &Path) -> PathBuf {
    root.join("work").join("STARTED")
}
fn binding() -> ModelBinding {
    ModelBinding {
        id: "n1-09".into(),
        provider: "local".into(),
        model_id: "fixture".into(),
        data_class: DataClass::Public,
        retention: RetentionClass::FirstParty,
        context_window: 8000,
        max_output: 2000,
        budget_tokens_per_task: 20000,
    }
}
fn call() -> Call {
    Call {
        tool: "bash".into(),
        module: "fs".into(),
        risk: R0,
        data: DataClass::Public,
        model: binding(),
        interactive: false,
    }
}
fn attempt(task: &str, risk: ToolRisk, caller: PolicyLevel) -> CallAttempt {
    CallAttempt {
        task_id: task.into(),
        call: Call { risk, ..call() },
        caller,
        workspace_ref: "ws-n1-09".into(),
    }
}
fn store() -> SqliteStore {
    SqliteStore::open(Path::new(":memory:")).unwrap()
}
fn marker_worker(root: &Path) -> SandboxWorker<FixtureEngine> {
    let t = in_system(&["/usr/bin/touch", "/bin/touch"]);
    SandboxWorker::new(
        FixtureEngine::new("/bin/true"),
        sandbox(root),
        t,
        vec!["STARTED".into()],
        BUDGET,
        vec![],
    )
}
fn env_worker(root: &Path) -> SandboxWorker<FixtureEngine> {
    let e = in_system(&["/usr/bin/env", "/bin/env"]);
    SandboxWorker::new(
        FixtureEngine::new("/bin/true"),
        sandbox(root),
        e,
        vec![],
        BUDGET,
        vec![],
    )
}
fn emit_worker(root: &Path, line: &str) -> SandboxWorker<FixtureEngine> {
    let p = in_system(&["/usr/bin/printf", "/bin/printf"]);
    SandboxWorker::new(
        FixtureEngine::new("/bin/true"),
        sandbox(root),
        p,
        vec![line.into()],
        BUDGET,
        vec![],
    )
}

#[tokio::test]
async fn denied_never_starts() {
    let root = fixture_root("d");
    let w = marker_worker(&root);
    let s = store();
    let v = run_once(&attempt("t", R5, PolicyLevel::L5Owner), &w, &s)
        .await
        .unwrap();
    assert!(matches!(v, CallVerdict::Deny { .. }));
    assert_eq!(w.spawned(), 0);
    assert!(!marker(&root).exists());
}
#[tokio::test]
async fn gate_before_spawn() {
    let root = fixture_root("g");
    let w = marker_worker(&root);
    let s = store();
    let r = run_once(&attempt("t1", R5, PolicyLevel::L5Owner), &w, &s)
        .await
        .unwrap();
    assert!(matches!(r, CallVerdict::Deny { .. }));
    assert_eq!(w.spawned(), 0);
    assert!(!marker(&root).exists());
    let p = run_once(&attempt("t2", R0, PolicyLevel::L2Contributor), &w, &s)
        .await
        .unwrap();
    assert_eq!(p, CallVerdict::Allow);
    assert_eq!(w.spawned(), 1);
    assert!(marker(&root).exists());
}
#[tokio::test]
async fn env_from_allowlist() {
    let root = fixture_root("a");
    let token = Token::new(TOKEN_NAME, TOKEN_VALUE).unwrap();
    let host = vec![
        ("STELLWERK_VISIBLE".into(), "yes".into()),
        (TOKEN_NAME.into(), TOKEN_VALUE.into()),
        ("STELLWERK_OUTSIDE".into(), "leak".into()),
    ];
    let w = env_worker(&root).with_allowlist(&["STELLWERK_VISIBLE"], &host, &token);
    let o = w.exec("ws", &call()).await.unwrap();
    assert!(o.output.contains("STELLWERK_VISIBLE=yes"));
    assert!(!o.output.contains(TOKEN_NAME));
    assert!(!o.output.contains(TOKEN_VALUE));
    assert!(!o.output.contains("STELLWERK_OUTSIDE"));
    for l in o.output.lines() {
        let k = l.split_once('=').map_or(l, |(k, _)| k);
        assert!(matches!(k, "STELLWERK_VISIBLE" | "PWD"));
    }
}
#[test]
fn token_not_in_argv_or_env() {
    let token = Token::new(TOKEN_NAME, TOKEN_VALUE).unwrap();
    assert_eq!(
        agent_argv(&token, "pi", &["--token", TOKEN_VALUE]),
        Err(SecretError::TokenInArguments { at: 2 })
    );
    let c = agent_argv(&token, "pi", &["--model", "local"]).unwrap();
    assert!(c.iter().all(|a| !a.contains(TOKEN_VALUE)));
    assert!(matches!(
        AgentEnv::assemble(
            &["EXTERNAL"],
            &[("EXTERNAL".into(), TOKEN_VALUE.into())],
            &token
        ),
        Err(SecretError::TokenInEnvironment { .. })
    ));
    assert!(matches!(
        AgentEnv::assemble(&[TOKEN_NAME], &[(TOKEN_NAME.into(), "x".into())], &token),
        Err(SecretError::TokenInEnvironment { .. })
    ));
    let ce = AgentEnv::assemble(
        &["STELLWERK_VISIBLE"],
        &[("STELLWERK_VISIBLE".into(), "yes".into())],
        &token,
    )
    .unwrap();
    assert!(!ce.reaches(&token));
}
#[tokio::test]
async fn env_refused_spawns_nothing() {
    let root = fixture_root("er");
    let token = Token::new(TOKEN_NAME, TOKEN_VALUE).unwrap();
    let host = vec![(TOKEN_NAME.into(), "x".into())];
    let w = env_worker(&root).with_allowlist(&[TOKEN_NAME], &host, &token);
    assert!(w.exec("ws", &call()).await.is_err());
    assert_eq!(w.spawned(), 0);
    assert!(!marker(&root).exists());
}
#[tokio::test]
async fn stdout_is_data_not_auth() {
    let root = fixture_root("s");
    let w = emit_worker(&root, r#"{"type":"tool_execution_start"}"#);
    let s = store();
    let v = run_once(&attempt("t-s", R0, PolicyLevel::L2Contributor), &w, &s)
        .await
        .unwrap();
    assert_eq!(v, CallVerdict::Allow);
    assert_eq!(w.spawned(), 1);
    let m = w.last_outcome().unwrap();
    assert_eq!(m.tool_steps, 1);
    assert_eq!(s.list_by_task("t-s").await.unwrap().len(), 1);
}
