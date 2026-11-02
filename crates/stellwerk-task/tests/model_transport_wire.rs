use std::io::{BufRead, BufReader, Error, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use stellwerk_task::m5::model_transport::{
    BackendError, ErrorCode, Frame, ModelBackend, ModelProxy, Served, TurnMessage, TurnRequest,
    MAX_FRAME_BYTES,
};

struct Scripted {
    frames: Vec<Frame>,
    calls: AtomicUsize,
}

impl ModelBackend for Scripted {
    fn turn(&self, _messages: &[TurnMessage]) -> Result<Vec<Frame>, BackendError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.frames.clone())
    }

    fn turn_request(&self, request: &TurnRequest) -> Result<Vec<Frame>, BackendError> {
        self.turn(&request.messages)
    }
}

struct Failing;

impl ModelBackend for Failing {
    fn turn(&self, _messages: &[TurnMessage]) -> Result<Vec<Frame>, BackendError> {
        Err(BackendError {
            message: "no route configured".to_owned(),
        })
    }

    fn turn_request(&self, request: &TurnRequest) -> Result<Vec<Frame>, BackendError> {
        self.turn(&request.messages)
    }
}

/// Запоминает имена предложенных инструментов: сам `TurnRequest` не `Clone`.
struct Recording {
    frames: Vec<Frame>,
    offered: Mutex<Vec<Vec<String>>>,
}

impl Recording {
    fn new(frames: Vec<Frame>) -> Arc<Self> {
        Arc::new(Self {
            frames,
            offered: Mutex::new(Vec::new()),
        })
    }

    fn offered(&self) -> Vec<Vec<String>> {
        self.offered.lock().expect("журнал предложений").clone()
    }
}

impl ModelBackend for Recording {
    fn turn(&self, _messages: &[TurnMessage]) -> Result<Vec<Frame>, BackendError> {
        Ok(self.frames.clone())
    }

    fn turn_request(&self, request: &TurnRequest) -> Result<Vec<Frame>, BackendError> {
        let names = request.tools.as_ref().map_or_else(Vec::new, |tools| {
            tools.iter().map(|tool| tool.name.clone()).collect()
        });
        self.offered.lock().expect("журнал предложений").push(names);
        Ok(self.frames.clone())
    }
}

/// Реализует только `turn`: проверяет умолчание `turn_request`.
struct TurnOnly;

impl ModelBackend for TurnOnly {
    fn turn(&self, _messages: &[TurnMessage]) -> Result<Vec<Frame>, BackendError> {
        Ok(vec![])
    }
}

fn bash_call() -> Frame {
    Frame::ToolCall {
        id: "call-1".to_owned(),
        name: "bash".to_owned(),
        input: serde_json::json!({ "command": "true" }),
    }
}

fn scripted(frames: Vec<Frame>) -> Arc<Scripted> {
    Arc::new(Scripted {
        frames,
        calls: AtomicUsize::new(0),
    })
}

fn proxy_at(tag: &str) -> Arc<ModelProxy> {
    let dir: PathBuf = std::env::temp_dir().join(format!("sw-n2d-{tag}-{}", std::process::id()));
    Arc::new(ModelProxy::bind(&dir).expect("прокси слушает сокет"))
}

fn serve(
    proxy: &Arc<ModelProxy>,
    backend: Arc<dyn ModelBackend>,
) -> std::thread::JoinHandle<Result<Served, Error>> {
    let proxy = Arc::clone(proxy);
    std::thread::spawn(move || proxy.serve_once(backend.as_ref()))
}

fn served(handle: std::thread::JoinHandle<Result<Served, Error>>) -> Served {
    handle
        .join()
        .expect("поток сервера")
        .expect("прокси не упал")
}

fn connect(proxy: &ModelProxy, payload: &[u8]) -> UnixStream {
    let mut stream = UnixStream::connect(proxy.socket()).expect("клиент подключается");
    stream.write_all(payload).expect("полезное записано");
    stream.shutdown(Shutdown::Write).expect("запись закрыта");
    stream
}

fn read_frames(stream: UnixStream) -> Vec<Frame> {
    BufReader::new(stream)
        .lines()
        .map(|line| serde_json::from_str(&line.expect("ответ читается")).expect("кадр — JSON"))
        .collect()
}

fn exchange(proxy: &ModelProxy, payload: &[u8]) -> Vec<Frame> {
    read_frames(connect(proxy, payload))
}

fn text(value: &str) -> Frame {
    Frame::Text {
        text: value.to_owned(),
    }
}

fn error_code(frames: &[Frame]) -> ErrorCode {
    match frames {
        [Frame::Error { code, .. }] => *code,
        other => panic!("ожидался ровно один error-кадр, а не {other:?}"),
    }
}

const TURN: &[u8] = b"{\"kind\":\"turn\",\"tools\":[{\"name\":\"bash\",\"description\":\"shell\",\"parameters\":{\"type\":\"object\"}}],\"messages\":[{\"role\":\"user\",\"text\":\"hi\"}]}\n";
const NO_TOOLS: &[u8] = b"{\"kind\":\"turn\",\"messages\":[{\"role\":\"user\",\"text\":\"hi\"}]}\n";
const EMPTY_TOOLS: &[u8] =
    b"{\"kind\":\"turn\",\"tools\":[],\"messages\":[{\"role\":\"user\",\"text\":\"hi\"}]}\n";

#[test]
fn a_turn_roundtrip_streams_text_tool_and_done() {
    let proxy = proxy_at("roundtrip");
    let backend = scripted(vec![
        text("привет"),
        Frame::ToolCall {
            id: "call-1".to_owned(),
            name: "bash".to_owned(),
            input: serde_json::json!({ "command": "true" }),
        },
    ]);
    let handle = serve(&proxy, Arc::clone(&backend) as Arc<dyn ModelBackend>);
    let frames = exchange(&proxy, TURN);

    assert_eq!(served(handle), Served::Turn);
    assert_eq!(backend.calls.load(Ordering::SeqCst), 1);
    assert_eq!(frames.len(), 3);
    assert!(matches!(&frames[0], Frame::Text { text } if text == "привет"));
    assert!(matches!(&frames[1], Frame::ToolCall { name, .. } if name == "bash"));
    assert!(matches!(&frames[2], Frame::Done { reason } if reason == "tool_use"));
}

#[test]
fn a_turn_without_tools_is_refused() {
    let proxy = proxy_at("no-tools");
    let backend = scripted(vec![text("привет")]);
    let handle = serve(&proxy, Arc::clone(&backend) as Arc<dyn ModelBackend>);
    let frames = exchange(&proxy, NO_TOOLS);

    assert_eq!(served(handle), Served::Refused);
    assert_eq!(backend.calls.load(Ordering::SeqCst), 0);
    assert_eq!(error_code(&frames), ErrorCode::Schema);
    let Frame::Error { message, .. } = &frames[0] else {
        panic!("ожидался error-кадр, а не {frames:?}");
    };
    assert!(
        message.contains("turn has no `tools` field"),
        "причина не названа: {message}"
    );
}

#[test]
fn an_empty_tool_set_is_an_explicit_choice() {
    let proxy = proxy_at("empty-tools-choice");
    let backend = scripted(vec![text("привет")]);
    let handle = serve(&proxy, Arc::clone(&backend) as Arc<dyn ModelBackend>);
    let frames = exchange(&proxy, EMPTY_TOOLS);

    assert_eq!(served(handle), Served::Turn);
    assert_eq!(backend.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        frames,
        vec![
            text("привет"),
            Frame::Done {
                reason: "stop".to_owned()
            }
        ]
    );
}

#[test]
fn an_unknown_field_is_still_refused_first() {
    let proxy = proxy_at("extra-before-tools");
    let backend = scripted(vec![]);
    let handle = serve(&proxy, Arc::clone(&backend) as Arc<dyn ModelBackend>);
    let frames = exchange(
        &proxy,
        b"{\"kind\":\"turn\",\"messages\":[],\"url\":\"http://x.invalid\"}\n",
    );

    assert_eq!(served(handle), Served::Refused);
    assert_eq!(backend.calls.load(Ordering::SeqCst), 0);
    assert_eq!(error_code(&frames), ErrorCode::UnexpectedField);
}

#[test]
fn an_explicit_empty_tool_set_forwards_the_call() {
    let proxy = proxy_at("empty-tools");
    let backend = scripted(vec![bash_call()]);
    let handle = serve(&proxy, Arc::clone(&backend) as Arc<dyn ModelBackend>);
    let frames = exchange(
        &proxy,
        b"{\"kind\":\"turn\",\"tools\":[],\"messages\":[{\"role\":\"user\",\"text\":\"hi\"}]}\n",
    );

    assert_eq!(served(handle), Served::Turn);
    assert_eq!(backend.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        frames,
        vec![
            bash_call(),
            Frame::Done {
                reason: "tool_use".to_owned()
            }
        ]
    );
}

#[test]
fn a_call_outside_the_offered_set_is_forwarded() {
    let proxy = proxy_at("outside-offer");
    let backend = Recording::new(vec![text("сначала текст"), bash_call()]);
    let handle = serve(&proxy, Arc::clone(&backend) as Arc<dyn ModelBackend>);
    let frames = exchange(
        &proxy,
        b"{\"kind\":\"turn\",\"tools\":[{\"name\":\"read\",\"description\":\"read\",\"parameters\":{}}],\"messages\":[{\"role\":\"user\",\"text\":\"hi\"}]}\n",
    );

    assert_eq!(served(handle), Served::Turn);
    assert_eq!(backend.offered(), vec![vec!["read".to_owned()]]);
    assert_eq!(frames.len(), 3);
    assert_eq!(frames[0], text("сначала текст"));
    assert!(matches!(&frames[1], Frame::ToolCall { name, .. } if name == "bash"));
    assert!(matches!(&frames[2], Frame::Done { reason } if reason == "tool_use"));
}

#[test]
fn a_call_inside_the_offered_set_is_forwarded() {
    let proxy = proxy_at("inside-offer");
    let backend = Recording::new(vec![text("привет"), bash_call()]);
    let handle = serve(&proxy, Arc::clone(&backend) as Arc<dyn ModelBackend>);
    let frames = exchange(
        &proxy,
        b"{\"kind\":\"turn\",\"tools\":[{\"name\":\"bash\",\"description\":\"shell\",\"parameters\":{}}],\"messages\":[{\"role\":\"user\",\"text\":\"hi\"}]}\n",
    );

    assert_eq!(served(handle), Served::Turn);
    assert_eq!(backend.offered(), vec![vec!["bash".to_owned()]]);
    assert_eq!(frames.len(), 3);
    assert!(matches!(&frames[2], Frame::Done { reason } if reason == "tool_use"));
}

#[test]
fn a_text_only_turn_still_ends_with_stop() {
    let proxy = proxy_at("text-only");
    let backend = scripted(vec![text("только текст")]);
    let handle = serve(&proxy, Arc::clone(&backend) as Arc<dyn ModelBackend>);
    let frames = exchange(&proxy, TURN);

    assert_eq!(served(handle), Served::Turn);
    assert_eq!(frames.len(), 2);
    assert!(matches!(&frames[1], Frame::Done { reason } if reason == "stop"));
}

#[test]
fn a_backend_without_tool_support_refuses_offered_tools() {
    let proxy = proxy_at("turn-only");
    let handle = serve(&proxy, Arc::new(TurnOnly));
    let frames = exchange(
        &proxy,
        b"{\"kind\":\"turn\",\"tools\":[{\"name\":\"read\",\"description\":\"read\",\"parameters\":{}}],\"messages\":[]}\n",
    );

    assert_eq!(served(handle), Served::Refused);
    assert_eq!(error_code(&frames), ErrorCode::BackendError);
}

#[test]
fn tool_definitions_are_validated_before_the_backend() {
    let payloads = [
        b"{\"kind\":\"turn\",\"tools\":[{\"name\":\"\",\"description\":\"read\",\"parameters\":{}}],\"messages\":[]}\n".as_slice(),
        b"{\"kind\":\"turn\",\"tools\":[{\"name\":\"read\",\"description\":\"read\",\"parameters\":[]}],\"messages\":[]}\n".as_slice(),
    ];
    for (index, payload) in payloads.into_iter().enumerate() {
        let proxy = proxy_at(&format!("tool-schema-{index}"));
        let backend = scripted(vec![]);
        let handle = serve(&proxy, Arc::clone(&backend) as Arc<dyn ModelBackend>);
        let frames = exchange(&proxy, payload);

        assert_eq!(served(handle), Served::Refused);
        assert_eq!(backend.calls.load(Ordering::SeqCst), 0);
        assert_eq!(error_code(&frames), ErrorCode::Schema);
    }
}

#[test]
fn a_null_tool_set_is_schema_not_absence() {
    let proxy = proxy_at("null-tools");
    let backend = scripted(vec![]);
    let handle = serve(&proxy, Arc::clone(&backend) as Arc<dyn ModelBackend>);
    let frames = exchange(
        &proxy,
        b"{\"kind\":\"turn\",\"tools\":null,\"messages\":[]}\n",
    );

    assert_eq!(served(handle), Served::Refused);
    assert_eq!(backend.calls.load(Ordering::SeqCst), 0);
    assert_eq!(error_code(&frames), ErrorCode::Schema);
}

#[test]
fn an_unknown_kind_is_refused_not_ignored() {
    let proxy = proxy_at("unknown-kind");
    let backend = scripted(vec![]);
    let handle = serve(&proxy, Arc::clone(&backend) as Arc<dyn ModelBackend>);
    let frames = exchange(&proxy, b"{\"kind\":\"shell\",\"command\":\"id\"}\n");

    assert_eq!(served(handle), Served::Refused);
    assert_eq!(backend.calls.load(Ordering::SeqCst), 0);
    assert_eq!(error_code(&frames), ErrorCode::UnknownKind);
}

#[test]
fn a_route_field_is_refused() {
    for field in ["url", "headers", "base_url", "model"] {
        let proxy = proxy_at(&format!("route-{field}"));
        let handle = serve(&proxy, Arc::new(Failing));
        let payload =
            format!("{{\"kind\":\"turn\",\"messages\":[],\"{field}\":\"http://x.invalid\"}}\n");
        let frames = exchange(&proxy, payload.as_bytes());

        assert_eq!(served(handle), Served::Refused);
        assert_eq!(error_code(&frames), ErrorCode::UnexpectedField);
        let Frame::Error { message, .. } = &frames[0] else {
            panic!("ожидался error-кадр, а не {frames:?}");
        };
        assert!(
            message.contains(field),
            "поле `{field}` не названо: {message}"
        );
    }
}

#[test]
fn malformed_json_is_refused() {
    let proxy = proxy_at("malformed");
    let handle = serve(&proxy, Arc::new(Failing));
    let frames = exchange(&proxy, b"{\n");

    assert_eq!(served(handle), Served::Refused);
    assert_eq!(error_code(&frames), ErrorCode::MalformedJson);
}

#[test]
fn an_oversize_frame_is_refused_before_it_is_buffered() {
    let proxy = proxy_at("oversize");
    let handle = serve(&proxy, Arc::new(Failing));
    let mut payload = b"{\"kind\":\"turn\",\"messages\":[]}".to_vec();
    payload.resize(MAX_FRAME_BYTES + 1, b'a');
    let stream = connect(&proxy, &payload);

    assert_eq!(served(handle), Served::Refused);
    assert_eq!(error_code(&read_frames(stream)), ErrorCode::OversizeFrame);
}

#[test]
fn a_frame_at_the_size_limit_is_served_not_refused() {
    let proxy = proxy_at("at-limit");
    let backend = scripted(vec![text("предел")]);
    let handle = serve(&proxy, Arc::clone(&backend) as Arc<dyn ModelBackend>);
    let mut payload = b"{\"kind\":\"turn\",\"tools\":[],\"messages\":[]}".to_vec();
    payload.resize(512 * 1024 - 1, b' ');
    payload.push(b'\n');
    let frames = exchange(&proxy, &payload);

    assert_eq!(served(handle), Served::Turn);
    assert_eq!(backend.calls.load(Ordering::SeqCst), 1);
    assert!(matches!(frames.last(), Some(Frame::Done { .. })));
}

#[test]
fn a_backend_error_is_an_error_frame_not_done() {
    let proxy = proxy_at("backend-error");
    let handle = serve(&proxy, Arc::new(Failing));
    let frames = exchange(&proxy, TURN);

    assert_eq!(served(handle), Served::Refused);
    assert_eq!(error_code(&frames), ErrorCode::BackendError);
}

#[test]
fn a_connection_without_a_request_is_no_request() {
    let proxy = proxy_at("no-request");
    let handle = serve(&proxy, Arc::new(Failing));
    let frames = exchange(&proxy, b"");

    assert_eq!(served(handle), Served::NoRequest);
    assert!(
        frames.is_empty(),
        "на пустое соединение кадров нет: {frames:?}"
    );
}

#[test]
fn a_second_turn_on_one_connection_is_not_served() {
    let proxy = proxy_at("second-turn");
    let backend = scripted(vec![text("один")]);
    let handle = serve(&proxy, Arc::clone(&backend) as Arc<dyn ModelBackend>);
    let frames = exchange(
        &proxy,
        b"{\"kind\":\"turn\",\"tools\":[],\"messages\":[{\"role\":\"user\",\"text\":\"1\"}]}\n\
          {\"kind\":\"turn\",\"tools\":[],\"messages\":[{\"role\":\"user\",\"text\":\"2\"}]}\n",
    );

    assert_eq!(served(handle), Served::Turn);
    assert_eq!(backend.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        frames
            .iter()
            .filter(|frame| matches!(frame, Frame::Done { .. }))
            .count(),
        1
    );
}

#[test]
fn a_dropped_proxy_removes_its_socket_file() {
    let proxy = proxy_at("drop");
    let socket = proxy.socket().to_path_buf();
    assert!(socket.exists(), "до уничтожения сокет на месте");
    drop(proxy);
    assert!(!socket.exists(), "сокет удалён вместе с прокси");
}
