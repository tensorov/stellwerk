//! N2-00f: Pi в `bwrap --unshare-net`; нет `pi`/`bwrap` — падение, нет гейта — нет эффекта.
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;
use stellwerk_core::contract::{DataClass, PolicyLevel, RiskClass};
use stellwerk_gateway::gate::CallRequest;
use stellwerk_gateway::permission::ToolRisk;
use stellwerk_task::m5::gate_channel::{GateChannel, GatePolicy};
use stellwerk_task::m5::model_transport::{
    BackendError, Frame, ModelBackend, ModelProxy, TurnMessage,
};
use stellwerk_task::m5::sandbox_bwrap::Sandbox;
use stellwerk_task::m5::session_identity::{
    Adjudication, CodemodeState, SessionDescription, ToolSet,
};
use stellwerk_task::m5::FixtureEngine;
use stellwerk_task::orchestrator::{pi_transport_args, SandboxWorker};
use stellwerk_task::runner::{Call, Worker};
const WINDOW: Duration = Duration::from_secs(60);
const BUDGET: Duration = Duration::from_secs(30);
const EXTENSION: &str = "pi/stellwerk_transport.ts";
static SEQ: AtomicUsize = AtomicUsize::new(0);
fn scratch(name: &str) -> PathBuf {
    let seq = SEQ.fetch_add(1, Ordering::SeqCst);
    let root = std::env::temp_dir().join(format!("sw-n2f-{name}-{}-{seq}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for dir in ["work", "tmp", "work/home", "sock"] {
        std::fs::create_dir_all(root.join(dir)).expect("каталог пробега");
    }
    root
}
fn worker(name: &str) -> (SandboxWorker<FixtureEngine>, PathBuf) {
    let root = scratch(name);
    let sandbox = Sandbox {
        work: root.join("work"),
        tmp: root.join("tmp"),
        read_only: Vec::new(),
        window: WINDOW,
    };
    let worker = SandboxWorker::new(
        FixtureEngine::new("/bin/true"),
        sandbox,
        "/bin/true",
        Vec::new(),
        BUDGET,
        Vec::new(),
    );
    (worker, root)
}
#[test]
fn pi_args_carry_one_extension_and_no_key() {
    let args = pi_transport_args(Path::new("/run/sw/ext.ts"), &["bash", "read"]);
    assert_eq!(
        args.join(" "),
        "--offline --no-session --no-extensions --no-context-files -e /run/sw/ext.ts --tools bash,read"
    );
}
#[test]
fn two_sockets_must_share_one_directory() {
    let (w, root) = worker("same");
    let (model, gate) = (root.join("sock/model.sock"), root.join("sock/gate.sock"));
    assert!(w.with_sockets(model, gate).is_ok());
    let (w, root) = worker("apart");
    let (model, gate) = (root.join("sock/model.sock"), root.join("other/gate.sock"));
    assert!(w.with_sockets(model, gate).is_err());
}
fn call(risk: ToolRisk, interactive: bool) -> Call {
    Call {
        tool: "bash".to_string(),
        module: "stellwerk-task".to_string(),
        risk,
        data: DataClass::Public,
        model: serde_json::from_str(r#"{"id":"b1","provider":"neuraldeep","model_id":"qwen3.6-unlim-xl-noreason","data_class":"public","retention":"first_party","context_window":138336,"max_output":41500,"budget_tokens_per_task":100000}"#).expect("binding"),
        interactive,
    }
}
struct Scripted(Vec<Frame>);
impl ModelBackend for Scripted {
    fn turn(&self, _messages: &[TurnMessage]) -> Result<Vec<Frame>, BackendError> {
        Ok(self.0.clone())
    }
}
struct Policy(Call);
impl GatePolicy for Policy {
    fn request_for(&self, call: &Adjudication) -> Option<CallRequest> {
        let held = &self.0;
        Some(CallRequest {
            tool: call.tool.clone(),
            module: held.module.clone(),
            risk: held.risk,
            caller: PolicyLevel::L5Owner,
            data: held.data,
            model: held.model.clone(),
            interactive: held.interactive,
        })
    }
}
// Внутрь идут только исполняемое дерево `pi` и системные каталоги; `auth.json` лежит
// рядом с `bin`, но НЕ монтируется (монтируются `bin` и `install`).
fn pi() -> (PathBuf, Vec<PathBuf>) {
    let home = std::env::var_os("HOME").expect("HOME задан");
    let launcher = std::env::var_os("PI_BIN").map_or_else(
        || PathBuf::from(home).join(".pi/agent/bin/pi"),
        PathBuf::from,
    );
    assert!(
        launcher.is_file(),
        "нет {}; задайте PI_BIN",
        launcher.display()
    );
    let bin = launcher.parent().expect("каталог launcher'а").to_path_buf();
    let install = bin.parent().expect("каталог агента").join("install");
    let mut roots = ["/usr", "/bin", "/lib", "/lib64", "/etc"]
        .map(PathBuf::from)
        .to_vec();
    roots.extend([bin, install]);
    (launcher, roots)
}
fn case(name: &str, tool: bool, risk: Option<RiskClass>, held: &[&str]) -> (String, bool) {
    let root = scratch(name);
    let (sock, work, tmp) = (root.join("sock"), root.join("work"), root.join("tmp"));
    let canary = work.join("canary");
    let model = ModelProxy::bind(&sock).expect("канал модели");
    let gate = risk.map(|_| GateChannel::bind(&sock).expect("канал гейта"));
    let extension = work.join("stellwerk_transport.ts");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(EXTENSION);
    std::fs::copy(source, &extension).expect("расширение скопировано");
    let mut args = pi_transport_args(&extension, &["bash", "read"]);
    let tail = [
        "--provider",
        "stellwerk",
        "--model",
        "proxy",
        "--mode",
        "json",
        "-p",
        "go",
    ];
    args.extend(tail.map(str::to_owned));
    let (launcher, read_only) = pi();
    let sandbox = Sandbox {
        work: work.clone(),
        tmp: tmp.clone(),
        read_only,
        window: WINDOW,
    };
    let worker = SandboxWorker::new(
        FixtureEngine::new("/bin/true"),
        sandbox,
        launcher,
        args,
        BUDGET,
        vec![
            ("HOME".into(), work.join("home").display().to_string()),
            ("PATH".into(), "/usr/bin:/bin".into()),
            ("TMPDIR".into(), tmp.display().to_string()),
            (
                "STELLWERK_MODEL_SOCK".into(),
                model.socket().display().to_string(),
            ),
            (
                "STELLWERK_GATE_SOCK".into(),
                sock.join("gate.sock").display().to_string(),
            ),
        ],
    )
    .with_sockets(model.socket(), sock.join("gate.sock"))
    .expect("один каталог сокетов");
    let first = if tool {
        Frame::ToolCall {
            id: "call-1".to_owned(),
            name: "bash".to_owned(),
            input: json!({ "command": format!("touch {}", canary.display()) }),
        }
    } else {
        Frame::Text {
            text: "pong".to_owned(),
        }
    };
    let mut frames = vec![vec![first]];
    if tool {
        frames.push(vec![Frame::Text {
            text: "ok".to_owned(),
        }]);
    }
    let mut servers: Vec<JoinHandle<()>> = vec![std::thread::spawn(move || {
        for frames in frames {
            model.serve_once(&Scripted(frames)).expect("ход модели");
        }
    })];
    if let (Some(gate), Some(class)) = (gate, risk) {
        let session = SessionDescription::new(
            "session-1",
            &std::env::temp_dir(),
            ToolSet::listed(held).expect("набор инструментов"),
            Duration::from_secs(60),
            CodemodeState::AbsentAfterConfigLoad,
        )
        .expect("описание сессии");
        servers.push(std::thread::spawn(move || {
            gate.serve_once(&session, &Policy(call(ToolRisk::Classified(class), true)))
                .expect("вердикт гейта");
        }));
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let request = call(ToolRisk::Classified(RiskClass::R2), false);
    let outcome = runtime
        .block_on(worker.exec("workspace", &request))
        .expect("pi");
    for server in servers {
        server.join().expect("сервер канала");
    }
    (outcome.output, canary.exists())
}
#[test]
fn pi_text_turn_through_socket_proxy() {
    let (text, _) = case("text", false, None, &["bash"]);
    assert!(text.contains("pong"), "stdout pi: {text:?}");
    assert!(text.contains("\"stopReason\":\"stop\""), "no stop");
}
#[test]
fn pi_tool_call_is_blocked_before_effect() {
    let (text, canary) = case("deny", true, Some(RiskClass::R5), &["bash"]);
    assert!(!canary, "canary не создан: {text:?}");
    assert!(text.contains("gate_denied"), "причина deny: {text:?}");
    assert!(text.contains("\"isError\":true"), "isError");
}
#[test]
fn pi_gate_unreachable_blocks() {
    let (text, canary) = case("unreachable", true, None, &["bash"]);
    assert!(!canary, "нет гейта — нет эффекта: {text:?}");
    assert!(text.contains("gate_unreachable"), "причина: {text:?}");
}
#[test]
fn pi_allowed_tool_executes() {
    let (text, canary) = case("allow", true, Some(RiskClass::R0), &["bash"]);
    assert!(canary, "allow исполняет: {text:?}");
}
#[test]
fn pi_tool_set_mismatch_refused() {
    let (text, canary) = case("unknown", true, Some(RiskClass::R0), &["read"]);
    assert!(!canary, "вне набора — блок: {text:?}");
    assert!(text.contains("gate_unknown_tool"), "причина: {text:?}");
}
