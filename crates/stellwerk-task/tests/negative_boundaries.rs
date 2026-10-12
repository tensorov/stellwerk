//! Девять обязательных отказов границы доверия — набор как один (M5/packet-10).
//!
//! Пакеты 07–09 ИНИЦИИРОВАЛИ эти проверки, а не завершили: каждая опирается на
//! отказ того пакета. Историческое «девять падений ДО защиты» не воспроизводится —
//! защиты на дереве, и снимать их ради красного прогона запрещено.

#[path = "negative_fixtures.rs"]
mod roster;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use roster::assert_named;
use stellwerk_core::contract::{DataClass, ModelBinding, PolicyLevel, RetentionClass, RiskClass};
use stellwerk_gateway::gate::{decide, CallRequest, Verdict};
use stellwerk_gateway::permission::ToolRisk;
use stellwerk_task::approval::{next_status, request};
use stellwerk_task::m5::sandbox_bwrap::Sandbox;
use stellwerk_task::m5::secrets::{AgentEnv, Token};
use stellwerk_task::m5::session_identity::{
    grant, CallSource, CodemodeState, SessionDescription, ToolSet,
};
use stellwerk_task::m5::{EngineAdapter, FixtureEngine};
use stellwerk_task::runner::CallVerdict;

const BUDGET: Duration = Duration::from_secs(30);
const TOKEN_NAME: &str = "GITHUB_TOKEN";
const TOKEN_VALUE: &str = "ghp-p10-never-in-the-agent-reach";
const MISSING: &str = "/var/tmp/stellwerk-packet-10/no-such-engine";

fn scratch(tag: &str) -> PathBuf {
    let named = format!("sw-p10-{tag}-{}", std::process::id());
    std::env::temp_dir().join(named)
}

fn session_of(tools: &[&str]) -> SessionDescription {
    let off = CodemodeState::AbsentAfterConfigLoad;
    let work = scratch("session");
    let set = ToolSet::listed(tools).expect("список без codemode");
    let built = SessionDescription::new("m5-p10", &work, set, BUDGET, off);
    built.expect("описание собрано")
}

/// Отказ 1 — `SandboxError::Outside` (`packet-07b`): путь ушёл за корень после `..`.
#[test]
fn write_outside() {
    let sandbox = Sandbox {
        work: scratch("outside"),
        tmp: scratch("outside-tmp"),
        read_only: Vec::new(),
        window: Duration::from_secs(60),
    };
    let marker = sandbox.work.join("marker");
    let escape = sandbox.work.join("../outside.txt");
    let built = sandbox.command(&escape, Path::new("/bin/true"), &[], BUDGET);
    let refused = built.unwrap_err();
    assert_named("write_outside", &format!("{refused:?}"));
    assert!(!marker.exists(), "процесса не было — работа не начата");
}

/// Отказ 2 — `SecretError::TokenInEnvironment` (`packet-09`): токен в окружении агента.
#[test]
fn token_in_agent_env() {
    let token = Token::new(TOKEN_NAME, TOKEN_VALUE).expect("токен задан");
    let host = vec![(TOKEN_NAME.to_owned(), TOKEN_VALUE.to_owned())];
    let built = AgentEnv::assemble(&[TOKEN_NAME], &host, &token);
    let refused = built.unwrap_err();
    assert_named("token_in_agent_env", &format!("{refused:?}"));
}

/// Отказ 3 — `Transition(Suspended)` (`packet-23`, В4): `Ask` — переход, не `Keep`.
#[test]
fn self_approval() {
    let risk = ToolRisk::Classified(RiskClass::R3);
    let asked = request("run-p10", "t-p10", "shell", risk);
    assert_eq!(asked.answerable_by, PolicyLevel::L5Owner);
    let effect = next_status(&CallVerdict::Ask, risk);
    let text = format!("{effect:?}: {}", asked.tool);
    assert_named("self_approval", &text);
}

/// Отказ 4 — `IdentityError::Escalation` (`packet-08c`): отказ ЦЕЛИКОМ, без остатка.
#[test]
fn subagent_escalation() {
    let parent = ToolSet::listed(&["read"]).expect("список без codemode");
    let built = grant(&parent, &["read", "write"]);
    let refused = built.unwrap_err();
    assert_named("subagent_escalation", &format!("{refused:?}"));
}

/// Отказ 5 — `Verdict::Suspended` (В4): публикация без ответа, а не `Allow`.
#[test]
fn publish_unanswered() {
    let model = ModelBinding {
        id: "b".to_owned(),
        provider: "neuraldeep".to_owned(),
        model_id: "m".to_owned(),
        data_class: DataClass::Public,
        retention: RetentionClass::FirstParty,
        context_window: 200_000,
        max_output: 8_000,
        budget_tokens_per_task: 43_000,
    };
    let req = CallRequest {
        tool: "publish".to_owned(),
        module: "notes".to_owned(),
        risk: ToolRisk::Classified(RiskClass::R2),
        caller: PolicyLevel::L3Maintainer,
        data: DataClass::Public,
        model,
        interactive: false,
    };
    let verdict = decide(&req);
    assert!(
        !matches!(verdict, Verdict::Allow),
        "идёт без ответа: {verdict:?}"
    );
    let text = format!("{verdict:?}: {}", req.tool);
    assert_named("publish_unanswered", &text);
}

/// Отказ 6 — `IdentityError::Truncated` (`packet-08a`): отказ несёт ОБА числа.
#[test]
fn content_not_swapped() {
    let built = ToolSet::exactly(2, &["read"]);
    let refused = built.unwrap_err();
    assert_named("content_not_swapped", &format!("{refused:?}"));
}

/// Отказ 7 — `EngineError::Spawn` (`packet-02a1`/`02a3`); контроль с ДОСТУПНЫМ
/// движком обязателен, иначе «метки нет» доказывала бы фикстуру, а не отказ.
#[test]
fn engine_unavailable() {
    let engine = FixtureEngine::new(MISSING);
    let report = engine.probe().expect("пустой причины не бывает");
    assert!(!report.is_available(), "нет программы — недоступен");
    let tmp = std::env::temp_dir();
    let tag = std::process::id();
    let shown = tmp.join(format!("sw-p10-shown-{tag}"));
    let mut touch = Command::new("/bin/touch");
    touch.arg(shown.display().to_string()).current_dir(&tmp);
    let spoke = FixtureEngine::new("/bin/touch").session(touch, BUDGET);
    assert!(shown.exists(), "контроль обязан оставить метку: {spoke:?}");
    let marker = tmp.join(format!("sw-p10-gone-{tag}"));
    let mut missing = Command::new(MISSING);
    missing.arg(marker.display().to_string()).current_dir(&tmp);
    let started = engine.session(missing, BUDGET);
    let refused = started.unwrap_err();
    assert_named("engine_unavailable", &format!("{refused:?}"));
    assert!(!marker.exists(), "процесса не было — работа не начата");
}

/// Отказ 8 — `IdentityError::NoSuchTool` (проба b; `packet-08a`/`08d`): набор исчерпывающий.
#[test]
fn mcp_config_detached() {
    let work = scratch("mcp");
    let config = work.join(".pi").join("mcp.json");
    std::fs::create_dir_all(work.join(".pi")).expect("каталог конфигурации");
    std::fs::write(&config, r#"{"mcpServers":{"polyforge":{}}}"#).expect("записано");
    let held = session_of(&["read"]).adjudicate("server_from_config", None);
    let refused = held.unwrap_err();
    assert_named("mcp_config_detached", &format!("{refused:?}"));
}

/// Отказ 9 — `IdentityError::UnrecognisedSource` (проба a, `packet-08d`): пустое
/// `parentToolCallId` — неопознанный источник, а не «значит прямой».
#[test]
fn codemode_recognised() {
    let session = session_of(&["read"]);
    let marked = Some("toolu_codemode");
    let nested = session.adjudicate("read", marked);
    assert!(nested.is_ok(), "вложенный вызов опознан");
    assert!(session.adjudicate("read", None).is_ok(), "прямой разрешён");
    let source = CallSource::of(Some("   "));
    let refused = source.unwrap_err();
    assert_named("codemode_recognised", &format!("{refused:?}"));
}

/// Реестр: удаление строки или функции ломает СБОРКУ, а не сокращает покрытие.
const REGISTERED: [(&str, fn()); roster::NINE] = [
    ("write_outside", write_outside),
    ("token_in_agent_env", token_in_agent_env),
    ("self_approval", self_approval),
    ("subagent_escalation", subagent_escalation),
    ("publish_unanswered", publish_unanswered),
    ("content_not_swapped", content_not_swapped),
    ("engine_unavailable", engine_unavailable),
    ("mcp_config_detached", mcp_config_detached),
    ("codemode_recognised", codemode_recognised),
];

/// П. 3: набор и перечень совпадают поимённо.
#[test]
fn roster_and_checks_agree() {
    let checks: Vec<&str> = REGISTERED.iter().map(|(id, _)| *id).collect();
    let ids: Vec<&str> = roster::ROSTER.iter().map(|(name, _)| *name).collect();
    assert_eq!(checks, ids, "набор разошёлся с перечнем");
}
