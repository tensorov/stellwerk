//! N2-00f: Pi в `bwrap --unshare-net`; нет `pi`/`bwrap` — падение, нет гейта — нет эффекта.
use serde_json::json;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;
use stellwerk_core::contract::{DataClass, PolicyLevel, RiskClass};
use stellwerk_gateway::gate::CallRequest;
use stellwerk_gateway::permission::ToolRisk;
use stellwerk_task::m5::gate_channel::{AllowBasis, GateChannel, GatePolicy, PolicyVerdict};
use stellwerk_task::m5::model_transport::{
    BackendError, Frame, ModelBackend, ModelProxy, TurnMessage, TurnRequest,
};
use stellwerk_task::m5::sandbox_bwrap::Sandbox;
use stellwerk_task::m5::session_identity::{
    Adjudication, CodemodeState, SessionDescription, ToolSet,
};
use stellwerk_task::m5::{EngineAdapter, EngineError, FixtureEngine, ProbeReport, SessionOutcome};
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
// `FixtureEngine` не вычитывает stdout, поэтому кадр больше трубы вешает Pi.
struct DrainingEngine;
impl EngineAdapter for DrainingEngine {
    fn probe(&self) -> Result<ProbeReport, EngineError> {
        ProbeReport::available("draining", "reads pipes while running")
    }
    fn session(
        &self,
        mut launch: Command,
        timeout: Duration,
    ) -> Result<SessionOutcome, EngineError> {
        let program = launch.get_program().to_string_lossy().into_owned();
        let mut child = launch
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|source| EngineError::Spawn {
                program: program.clone(),
                source,
            })?;
        let out = drain(child.stdout.take().expect("stdout"));
        let err = drain(child.stderr.take().expect("stderr"));
        let deadline = std::time::Instant::now() + timeout;
        let status = loop {
            let waited = child
                .try_wait()
                .map_err(|source| EngineError::Io { source })?;
            if let Some(status) = waited {
                break status;
            }
            if std::time::Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err(EngineError::TimedOut { program, timeout });
            }
            std::thread::sleep(Duration::from_millis(2));
        };
        Ok(SessionOutcome {
            code: status.code(),
            stdout: String::from_utf8_lossy(&out.join().expect("out")).into_owned(),
            stderr: String::from_utf8_lossy(&err.join().expect("err")).into_owned(),
        })
    }
}
fn drain(mut pipe: impl Read + Send + 'static) -> JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        pipe.read_to_end(&mut bytes).expect("read pipe");
        bytes
    })
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
type TurnRecord = (Option<Vec<String>>, Vec<TurnMessage>);
struct Scripted(Vec<Frame>, Arc<Mutex<Vec<TurnRecord>>>);
impl ModelBackend for Scripted {
    fn turn(&self, _messages: &[TurnMessage]) -> Result<Vec<Frame>, BackendError> {
        Ok(self.0.clone())
    }
    // Ход принимает предложенный набор: без этого умолчание трейта отвергло бы tools.
    fn turn_request(&self, request: &TurnRequest) -> Result<Vec<Frame>, BackendError> {
        let offered = request.tools.as_ref();
        let tools = offered.map(|set| set.iter().map(|tool| tool.name.clone()).collect());
        let mut seen = self.1.lock().expect("журнал ходов");
        seen.push((tools, request.messages.clone()));
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
    // Путь — след вызова, а не второй фильтр этой тестовой политики.
    fn decide_call(&self, call: &Adjudication, _path: Option<&str>) -> PolicyVerdict {
        let Some(request) = self.request_for(call) else {
            return PolicyVerdict::Unclassified;
        };
        PolicyVerdict::Evaluated {
            request,
            basis: AllowBasis::RiskTable,
        }
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
struct Run {
    output: String,
    canary: bool,
    work: PathBuf,
    seen: Arc<Mutex<Vec<TurnRecord>>>,
}
fn run<F>(name: &str, offered: &[&str], risk: Option<RiskClass>, held: &[&str], turns: F) -> Run
where
    F: FnOnce(&Path) -> Vec<Vec<Frame>>,
{
    let root = scratch(name);
    let (sock, work, tmp) = (root.join("sock"), root.join("work"), root.join("tmp"));
    let canary = work.join("canary");
    let model = ModelProxy::bind(&sock).expect("канал модели");
    let gate = risk.map(|_| GateChannel::bind(&sock).expect("канал гейта"));
    let extension = work.join("stellwerk_transport.ts");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(EXTENSION);
    std::fs::copy(source, &extension).expect("расширение скопировано");
    let mut args = pi_transport_args(&extension, offered);
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
        DrainingEngine,
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
    let frames = turns(&work);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let recorder = Arc::clone(&seen);
    let mut servers: Vec<JoinHandle<()>> = vec![std::thread::spawn(move || {
        for frames in frames {
            let contradictory = frames.iter().any(|f| matches!(f, Frame::Done { .. }));
            let served = model.serve_once(&Scripted(frames, Arc::clone(&recorder)));
            if let Err(error) = served {
                assert!(contradictory, "ход модели: {error}");
            }
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
    Run {
        output: outcome.output,
        canary: canary.exists(),
        work,
        seen,
    }
}
fn bash_turn(name: &str, tool: bool, risk: Option<RiskClass>, held: &[&str]) -> Run {
    run(name, &["bash", "read"], risk, held, |work| {
        let command = json!({ "command": format!("touch {}", work.join("canary").display()) });
        let call = tool_call("call-1", "bash", command);
        if tool {
            vec![vec![call], vec![text_frame("ok")]]
        } else {
            vec![vec![text_frame("pong")]]
        }
    })
}
#[test]
fn pi_text_turn_through_socket_proxy() {
    let run = bash_turn("text", false, None, &["bash"]);
    assert!(run.output.contains("pong"), "stdout pi: {}", run.output);
    assert!(run.output.contains("\"stopReason\":\"stop\""), "no stop");
}
#[test]
fn pi_tool_call_is_blocked_before_effect() {
    let run = bash_turn("deny", true, Some(RiskClass::R5), &["bash"]);
    assert!(!run.canary, "canary не создан: {}", run.output);
    let denied = run.output.contains("gate_denied");
    assert!(denied, "{}", run.output);
    assert!(run.output.contains("\"isError\":true"), "isError");
}
#[test]
fn pi_gate_unreachable_blocks() {
    let run = bash_turn("unreachable", true, None, &["bash"]);
    assert!(!run.canary, "нет гейта — нет эффекта: {}", run.output);
    let unreachable = run.output.contains("gate_unreachable");
    assert!(unreachable, "{}", run.output);
}
#[test]
fn pi_allowed_tool_executes() {
    let run = bash_turn("allow", true, Some(RiskClass::R0), &["bash"]);
    assert!(run.canary, "allow исполняет: {}", run.output);
}
#[test]
fn pi_tool_set_mismatch_refused() {
    let run = bash_turn("unknown", true, Some(RiskClass::R0), &["read"]);
    assert!(!run.canary, "вне набора — блок: {}", run.output);
    let unknown = run.output.contains("gate_unknown_tool");
    assert!(unknown, "{}", run.output);
}
fn text_frame(text: &str) -> Frame {
    Frame::Text {
        text: text.to_owned(),
    }
}
fn tool_call(id: &str, name: &str, input: serde_json::Value) -> Frame {
    Frame::ToolCall {
        id: id.to_owned(),
        name: name.to_owned(),
        input,
    }
}
fn read_turn(name: &str, risk: Option<RiskClass>) -> Run {
    run(
        name,
        &["read", "edit", "write"],
        risk,
        &["read", "edit", "write"],
        |work| {
            std::fs::write(work.join("note.txt"), "hello\n").expect("заметка");
            vec![
                vec![tool_call("call-1", "read", json!({ "path": "note.txt" }))],
                vec![text_frame("ok")],
            ]
        },
    )
}
#[test]
fn pi_offers_its_tools_on_every_turn() {
    let run = read_turn("offers", Some(RiskClass::R0));
    let seen = run.seen.lock().expect("журнал ходов");
    assert!(seen.len() > 1);
    for turn in seen.iter() {
        let tools = turn.0.as_ref().expect("tools на каждом ходу");
        assert_eq!(tools, &["read", "edit", "write"]);
        assert!(!tools.iter().any(|tool| tool == "bash"));
    }
}
#[test]
fn the_second_turn_carries_the_tool_result_with_its_id() {
    let run = read_turn("result", Some(RiskClass::R0));
    let seen = run.seen.lock().expect("журнал ходов");
    assert!(seen.len() > 1, "второго хода нет: {}", seen.len());
    let second = &seen[1];
    let carried = second.1.iter().any(|message| {
        matches!(message, TurnMessage::Tool { tool_call_id, name, .. }
            if tool_call_id == "call-1" && name == "read")
    });
    assert!(carried, "во втором ходе нет результата: {:?}", second.1);
}
#[test]
fn a_done_reason_that_contradicts_the_frames_is_an_error() {
    let run = run(
        "contradiction",
        &["read", "edit", "write"],
        None,
        &["read"],
        |_work| {
            vec![vec![
                tool_call(
                    "call-1",
                    "read",
                    json!({ "path": "stellwerk_transport.ts" }),
                ),
                Frame::Done {
                    reason: "stop".to_owned(),
                },
            ]]
        },
    );
    assert!(!run.canary, "вызов не исполнен");
    let errored = run.output.contains("\"stopReason\":\"error\"");
    assert!(errored, "{}", run.output);
    assert!(run.output.contains("contradicts"), "{}", run.output);
}
#[test]
fn a_large_inbound_frame_is_accepted() {
    let body = format!("BEGIN{}END", "x".repeat(64 * 1024 + 1));
    let run = run("large", &["read"], None, &["read"], |_work| {
        vec![vec![text_frame(&body)]]
    });
    assert!(run.output.len() > 64 * 1024, "кадр: {}", run.output.len());
    assert!(run.output.matches('x').count() > 64 * 1024, "тело усечено");
    assert!(!run.output.contains("frame exceeds limit"));
    assert!(run.output.contains("\"stopReason\":\"stop\""));
}
#[test]
fn pi_executes_an_edit_the_model_asked_for() {
    let run = run(
        "edit",
        &["read", "edit", "write"],
        Some(RiskClass::R1),
        &["read", "edit", "write"],
        |work| {
            std::fs::create_dir_all(work.join("src")).expect("src");
            std::fs::write(work.join("src/lib.rs"), "pub fn one() -> u8 { 1 }\n").expect("файл");
            let input = json!({
                "path": "src/lib.rs",
                "edits": [{ "oldText": "pub fn one() -> u8 { 1 }", "newText": "pub fn one() -> u8 { 2 }" }]
            });
            vec![
                vec![tool_call("call-1", "edit", input)],
                vec![text_frame("ok")],
            ]
        },
    );
    let edited = std::fs::read_to_string(run.work.join("src/lib.rs")).expect("файл на месте");
    assert!(edited.contains("{ 2 }"), "правка: {edited}");
    assert!(!run.output.contains("gate_denied"));
    assert!(run.seen.lock().expect("журнал ходов").len() > 1);
}
