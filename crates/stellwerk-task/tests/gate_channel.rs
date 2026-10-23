use std::io::{BufRead, BufReader, Error, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use stellwerk_core::contract::PolicyLevel::L5Owner;
use stellwerk_core::contract::RiskClass::{R0, R1, R2, R5};
use stellwerk_core::contract::{DataClass, ModelBinding, PolicyLevel, RetentionClass, RiskClass};
use stellwerk_gateway::gate::CallRequest;
use stellwerk_gateway::permission::ToolRisk;
use stellwerk_task::m5::gate_channel::{
    AllowBasis, GateAnswer, GateChannel, GatePolicy, GateServed, PolicyVerdict, RefusalCode,
};
use stellwerk_task::m5::model_transport::MAX_FRAME_BYTES;
use stellwerk_task::m5::session_identity::{
    Adjudication, CallSource, CodemodeState, SessionDescription, ToolSet,
};

struct Scripted {
    risk: Option<ToolRisk>,
    level: PolicyLevel,
    interactive: bool,
    calls: AtomicUsize,
    seen: Mutex<Vec<CallSource>>,
    paths: Mutex<Vec<Option<String>>>,
}

impl GatePolicy for Scripted {
    /// Записывает путь, который увидела политика, и решает по таблице рисков —
    /// ровно то, что делает продовая политика, кроме план-скоупа.
    fn decide_call(&self, call: &Adjudication, path: Option<&str>) -> PolicyVerdict {
        self.paths.lock().unwrap().push(path.map(str::to_owned));
        match self.request_for(call) {
            Some(request) => PolicyVerdict::Evaluated {
                request,
                basis: AllowBasis::RiskTable,
            },
            None => PolicyVerdict::Unclassified,
        }
    }

    fn request_for(&self, call: &Adjudication) -> Option<CallRequest> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.seen.lock().unwrap().push(call.source.clone());
        Some(CallRequest {
            tool: call.tool.clone(),
            module: "core".to_owned(),
            risk: self.risk?,
            caller: self.level,
            data: DataClass::Public,
            model: binding(),
            interactive: self.interactive,
        })
    }
}

fn binding() -> ModelBinding {
    ModelBinding {
        id: "b1".to_owned(),
        provider: "neuraldeep".to_owned(),
        model_id: "qwen3.6-unlim-xl-noreason".to_owned(),
        data_class: DataClass::Public,
        retention: RetentionClass::FirstParty,
        context_window: 138_336,
        max_output: 41_500,
        budget_tokens_per_task: 100_000,
    }
}

fn session() -> SessionDescription {
    SessionDescription::new(
        "session-1",
        &std::env::temp_dir(),
        ToolSet::listed(&["read"]).expect("набор инструментов"),
        Duration::from_secs(60),
        CodemodeState::AbsentAfterConfigLoad,
    )
    .expect("описание сессии")
}

static SEQ: AtomicUsize = AtomicUsize::new(0);

fn channel(tag: &str) -> Arc<GateChannel> {
    let seq = SEQ.fetch_add(1, Ordering::SeqCst);
    let dir: PathBuf = std::env::temp_dir().join(format!("sw-n2e-{tag}-{seq}"));
    let _ = std::fs::remove_dir_all(&dir);
    Arc::new(GateChannel::bind(&dir).expect("гейт слушает сокет"))
}

fn scripted(risk: Option<ToolRisk>, interactive: bool) -> Arc<Scripted> {
    Arc::new(Scripted {
        risk,
        level: L5Owner,
        interactive,
        calls: AtomicUsize::new(0),
        seen: Mutex::new(Vec::new()),
        paths: Mutex::new(Vec::new()),
    })
}

fn serve(
    gate: &Arc<GateChannel>,
    policy: Arc<dyn GatePolicy>,
) -> std::thread::JoinHandle<Result<GateServed, Error>> {
    let gate = Arc::clone(gate);
    std::thread::spawn(move || gate.serve_once(&session(), policy.as_ref()))
}

fn exchange(
    gate: &Arc<GateChannel>,
    policy: Arc<dyn GatePolicy>,
    payload: impl AsRef<[u8]>,
) -> (Vec<GateAnswer>, GateServed) {
    let handle = serve(gate, policy);
    let mut stream = UnixStream::connect(gate.socket()).expect("клиент подключается");
    stream.write_all(payload.as_ref()).expect("заявка записана");
    stream.shutdown(Shutdown::Write).expect("запись закрыта");
    let answers: Vec<GateAnswer> = BufReader::new(stream)
        .lines()
        .map(|line| serde_json::from_str(&line.expect("ответ читается")).expect("ответ — кадр"))
        .collect();
    let served = handle.join().expect("поток гейта").expect("гейт не упал");
    (answers, served)
}

fn only(answers: &[GateAnswer]) -> &GateAnswer {
    assert_eq!(answers.len(), 1, "ровно один ответ: {answers:?}");
    &answers[0]
}

fn refused(answers: &[GateAnswer]) -> (&RefusalCode, &str) {
    let GateAnswer::Refused { code, message } = only(answers) else {
        panic!("ожидался отказ: {answers:?}");
    };
    (code, message)
}

fn parent(answers: &[GateAnswer]) -> &str {
    match only(answers) {
        GateAnswer::UnrecognisedSource {
            parent_tool_call_id,
        } => parent_tool_call_id,
        other => panic!("ожидался unrecognised_source: {other:?}"),
    }
}

fn request(tool: &str, parent: Option<&str>) -> Vec<u8> {
    let parent = parent.map_or_else(|| "null".to_owned(), |id| format!("\"{id}\""));
    format!("{{\"kind\":\"tool_call\",\"tool\":\"{tool}\",\"parent_tool_call_id\":{parent}}}\n")
        .into_bytes()
}

struct Run {
    answers: Vec<GateAnswer>,
    policy: Arc<Scripted>,
}

fn run(risk: Option<RiskClass>, tool: &str, parent: Option<&str>, interactive: bool) -> Run {
    let gate = channel(&format!("{tool}-{}", parent.unwrap_or("direct")));
    let policy = scripted(risk.map(ToolRisk::Classified), interactive);
    let (answers, _) = exchange(&gate, policy.clone(), request(tool, parent));
    Run { answers, policy }
}

#[test]
fn an_allowed_tool_is_answered_allow() {
    let run = run(Some(R0), "read", None, true);
    assert!(matches!(&run.answers[..], [GateAnswer::Allow { tool }] if tool == "read"));
    assert_eq!(run.policy.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn a_policy_deny_is_deny_not_suspended() {
    let run = run(Some(R5), "read", None, true);
    let GateAnswer::Deny { reason, .. } = only(&run.answers) else {
        panic!("ожидался deny: {:?}", run.answers);
    };
    assert!(reason.contains("risk_denied"), "причина: {reason}");
}

#[test]
fn an_unknown_answer_is_never_allow() {
    let unclassified = run(None, "read", None, true);
    let m =
        matches!(&unclassified.answers[..], [GateAnswer::Unclassified { tool }] if tool == "read");
    assert!(m, "нет риска — не allow: {:?}", unclassified.answers);
    let suspended = run(Some(R2), "read", None, false);
    let m = matches!(&suspended.answers[..], [GateAnswer::Suspended { tool }] if tool == "read");
    assert!(m, "нет ответа — не allow: {:?}", suspended.answers);
}

#[test]
fn a_call_outside_the_tool_set_never_reaches_the_policy() {
    let run = run(Some(R0), "write", None, true);
    let GateAnswer::UnknownTool { tool, held } = only(&run.answers) else {
        panic!("ожидался unknown_tool: {:?}", run.answers);
    };
    assert_eq!((tool.as_str(), held), ("write", &vec!["read".to_owned()]));
    assert_eq!(run.policy.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn an_empty_parent_is_unrecognised_source() {
    let run = run(Some(R0), "read", Some(""), true);
    assert!(parent(&run.answers).is_empty(), "родитель пуст");
    assert_eq!(run.policy.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn a_nested_call_keeps_its_parent() {
    let run = run(Some(R0), "read", Some("call-7"), true);
    assert!(matches!(&run.answers[..], [GateAnswer::Allow { tool }] if tool == "read"));
    let seen = run.policy.seen.lock().unwrap().clone();
    assert_eq!(
        seen,
        vec![CallSource::Nested {
            parent_tool_call_id: "call-7".to_owned()
        }]
    );
}

#[test]
fn a_self_assigned_risk_is_refused_before_the_policy() {
    let gate = channel("self-risk");
    let policy = scripted(Some(ToolRisk::Classified(R0)), true);
    let payload = b"{\"kind\":\"tool_call\",\"tool\":\"read\",\"risk\":\"r0\"}\n";
    let (answers, served) = exchange(&gate, policy.clone(), payload);
    assert_eq!(served, GateServed::Refused);
    assert_eq!(*refused(&answers).0, RefusalCode::UnexpectedField);
    assert_eq!(policy.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn a_second_request_is_not_served() {
    let gate = channel("second");
    let mut payload = request("read", None);
    payload.extend(request("read", None));
    let policy = scripted(Some(ToolRisk::Classified(R0)), true);
    let (answers, served) = exchange(&gate, policy, &payload);
    assert_eq!(served, GateServed::Answered);
    assert!(matches!(&answers[..], [GateAnswer::Allow { tool }] if tool == "read"));
}

#[test]
fn a_connection_without_a_request_is_no_request() {
    let gate = channel("no-request");
    let (answers, served) = exchange(&gate, scripted(None, true), b"");
    assert_eq!(served, GateServed::NoRequest);
    assert!(
        answers.is_empty(),
        "на пустое соединение ответов нет: {answers:?}"
    );
}

#[test]
fn a_gate_frame_over_the_limit_is_refused() {
    let gate = channel("oversize");
    let mut payload = b"{\"kind\":\"tool_call\",\"tool\":\"read\"}".to_vec();
    payload.resize(MAX_FRAME_BYTES + 1, b'a');
    let (answers, served) = exchange(&gate, scripted(None, true), &payload);
    assert_eq!(served, GateServed::Refused);
    assert_eq!(*refused(&answers).0, RefusalCode::OversizeFrame);
}

/// Политика, которая не знает о путях: так выглядит любой существующий
/// реализатор. Путь в кадре для неё — данные, которые она не умеет толковать,
/// и умолчание обязано отказывать, а не пропускать.
struct Legacy;

impl GatePolicy for Legacy {
    fn request_for(&self, call: &Adjudication) -> Option<CallRequest> {
        Some(CallRequest {
            tool: call.tool.clone(),
            module: "core".to_owned(),
            risk: ToolRisk::Classified(R0),
            caller: L5Owner,
            data: DataClass::Public,
            model: binding(),
            interactive: true,
        })
    }
}

/// Политика, которая разрешает запись «по плану».
struct Planned {
    risk: RiskClass,
    interactive: bool,
    basis: Option<&'static str>,
}

impl GatePolicy for Planned {
    fn request_for(&self, _call: &Adjudication) -> Option<CallRequest> {
        None
    }

    fn decide_call(&self, call: &Adjudication, _path: Option<&str>) -> PolicyVerdict {
        PolicyVerdict::Evaluated {
            request: CallRequest {
                tool: call.tool.clone(),
                module: "core".to_owned(),
                risk: ToolRisk::Classified(self.risk),
                caller: L5Owner,
                data: DataClass::Public,
                model: binding(),
                interactive: self.interactive,
            },
            basis: match self.basis {
                Some(scope) => AllowBasis::ApprovedPlan {
                    scope: scope.to_owned(),
                },
                None => AllowBasis::RiskTable,
            },
        }
    }
}

fn request_with_path(tool: &str, path: &str) -> Vec<u8> {
    format!(
        "{{\"kind\":\"tool_call\",\"tool\":\"{tool}\",\"parent_tool_call_id\":null,\"path\":\"{path}\"}}\n"
    )
    .into_bytes()
}

#[test]
fn a_frame_carries_the_path_to_the_policy() {
    let gate = channel("path");
    let policy = scripted(Some(ToolRisk::Classified(R0)), true);
    let (answers, _) = exchange(
        &gate,
        policy.clone(),
        request_with_path("read", "src/lib.rs"),
    );
    assert!(
        matches!(&answers[..], [GateAnswer::Allow { tool }] if tool == "read"),
        "ответ: {answers:?}"
    );
    assert_eq!(
        policy.paths.lock().unwrap().as_slice(),
        &[Some("src/lib.rs".to_owned())],
        "политика видит путь из кадра"
    );
}

#[test]
fn a_frame_without_a_path_leaves_it_unknown() {
    let gate = channel("nopath");
    let policy = scripted(Some(ToolRisk::Classified(R0)), true);
    let _ = exchange(&gate, policy.clone(), request("read", None));
    assert_eq!(
        policy.paths.lock().unwrap().as_slice(),
        &[None],
        "нет пути — это None, а не пустая строка"
    );
}

#[test]
fn a_policy_that_did_not_opt_into_paths_refuses_them() {
    let gate = channel("legacy");
    let (answers, _) = exchange(
        &gate,
        Arc::new(Legacy),
        request_with_path("read", "src/lib.rs"),
    );
    let GateAnswer::Deny { reason, .. } = only(&answers) else {
        panic!("ожидался deny: {answers:?}");
    };
    assert!(
        reason.contains("policy_does_not_handle_paths"),
        "причина: {reason}"
    );
}

#[test]
fn a_policy_denial_is_deny_not_unclassified() {
    struct Refuser;
    impl GatePolicy for Refuser {
        fn request_for(&self, _call: &Adjudication) -> Option<CallRequest> {
            None
        }
        fn decide_call(&self, _call: &Adjudication, _path: Option<&str>) -> PolicyVerdict {
            PolicyVerdict::Refused {
                reason: "plan_scope_outside: src/other.rs".to_owned(),
            }
        }
    }
    let gate = channel("refused2");
    let (answers, _) = exchange(&gate, Arc::new(Refuser), request("read", None));
    let GateAnswer::Deny { reason, .. } = only(&answers) else {
        panic!("ожидался deny: {answers:?}");
    };
    assert!(
        reason.starts_with("plan_scope_outside"),
        "причина: {reason}"
    );
}

#[test]
fn an_approved_plan_basis_is_authorized_not_allow() {
    let gate = channel("authorized");
    let policy = Arc::new(Planned {
        risk: R1,
        interactive: false,
        basis: Some("src/lib.rs"),
    });
    let (answers, _) = exchange(&gate, policy, request("read", None));
    assert!(
        matches!(
            &answers[..],
            [GateAnswer::Authorized { tool, scope }]
                if tool == "read" && scope == "src/lib.rs"
        ),
        "ответ: {answers:?}"
    );
}

#[test]
fn a_plan_basis_does_not_override_ask() {
    let gate = channel("basis-ask");
    let policy = Arc::new(Planned {
        risk: R2,
        interactive: true,
        basis: Some("src/lib.rs"),
    });
    let (answers, _) = exchange(&gate, policy, request("read", None));
    assert!(
        matches!(&answers[..], [GateAnswer::Ask { .. }]),
        "основание не превращает Ask в Authorized: {answers:?}"
    );
}
