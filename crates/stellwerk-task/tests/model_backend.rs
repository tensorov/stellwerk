//! N2-00g3: ходовой backend говорит с OpenAI-совместимым сервисом по HTTP.
//!
//! Сервер здесь настоящий, но локальный: TLS проверяется отдельно и в этих тестах не
//! участвует — `http` разрешён только на loopback, и это тоже проверяется. Проверяется
//! форма запроса (путь, host, тип, тело) и разбор ответа: текст, вызовы инструментов и
//! отказы. Молчаливый пустой ход запрещён — «сервис ничего не сказал» и «сервис ответил
//! пустотой» обязаны быть разными исходами.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use stellwerk_task::m5::model_backend::HttpBackend;
use stellwerk_task::m5::model_transport::{
    AssistantCall, BackendError, ErrorCode, Frame, ModelBackend, ModelProxy, ToolDef, TurnMessage,
    TurnRequest,
};
use stellwerk_task::m5::secrets::{Token, TokenProxy};

const VALUE: &str = "nd-t0ken-for-backend-tests";
const MODEL: &str = "test-model";

fn backend(url: &str) -> HttpBackend {
    let token = TokenProxy::new(Token::new("NEURALDEEP_TOKEN", VALUE).expect("токен задан"));
    HttpBackend::new(url, MODEL, token).expect("backend собран")
}

fn user(text: &str) -> TurnMessage {
    TurnMessage::User {
        text: text.to_owned(),
    }
}

/// Читает запрос целиком по `content-length`: без него тело осталось бы в сокете, и сервер
/// проверял бы только заголовки.
fn read_request(stream: &mut TcpStream) -> String {
    let mut reader = BufReader::new(stream.try_clone().expect("клон сокета"));
    let mut head = String::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).expect("строка запроса");
        head.push_str(&line);
        if line == "\r\n" || line.is_empty() {
            break;
        }
    }
    let length: usize = head
        .lines()
        .find_map(|line| {
            line.to_lowercase()
                .strip_prefix("content-length:")
                .map(|value| value.trim().parse().expect("длина числом"))
        })
        .unwrap_or(0);
    let mut body = vec![0; length];
    reader.read_exact(&mut body).expect("тело запроса");
    head.push_str(&String::from_utf8_lossy(&body));
    head
}

/// Соединение принимается с потолком: backend, который никуда не пошёл, обязан провалить
/// тест за секунды, а не держать поток `accept` до таймаута всего прогона.
fn accept_within(listener: &TcpListener) -> TcpStream {
    listener.set_nonblocking(true).expect("неблокирующий слушатель");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match listener.accept() {
            Ok((stream, _)) => return stream,
            Err(why) if why.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < deadline, "backend не подключился за 5 с");
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(why) => panic!("соединение: {why}"),
        }
    }
}

/// Один запрос — один ответ. `worker` возвращает полученный запрос: утверждения о форме
/// делаются в тесте, а не внутри сервера, иначе падение было бы без текста запроса.
struct Served {
    url: String,
    worker: JoinHandle<String>,
}

fn serve(code: u16, body: impl FnOnce(&str) -> String + Send + 'static) -> Served {
    let listener = TcpListener::bind("127.0.0.1:0").expect("локальный порт");
    let port = listener.local_addr().expect("адрес").port();
    let url = format!("http://127.0.0.1:{port}/v1");
    let worker = std::thread::spawn(move || {
        let mut stream = accept_within(&listener);
        let request = read_request(&mut stream);
        let body = body(&request);
        let response = format!(
            "HTTP/1.1 {code} X\r\ncontent-type: application/json\r\n\
             content-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        stream
            .write_all(response.as_bytes())
            .expect("ответ записан");
        stream.flush().expect("ответ отправлен");
        request
    });
    Served { url, worker }
}

/// П. 1: ход уходит на `/chat/completions` с host, типом и телом, а роль пользователя
/// доезжает в теле, не превращаясь в строку запроса.
#[test]
fn a_text_turn_reaches_the_service_as_chat_completions() {
    let served = serve(200, |_| {
        r#"{"choices":[{"message":{"content":"pong"}}]}"#.to_owned()
    });
    let frames = backend(&served.url).turn(&[user("ping")]).expect("ход");
    let request = served.worker.join().expect("сервер завершился");

    assert!(
        request.starts_with("POST /v1/chat/completions HTTP/1.1\r\n"),
        "{request}"
    );
    assert!(request.contains("host: 127.0.0.1:"), "{request}");
    assert!(
        request.contains("content-type: application/json"),
        "{request}"
    );
    assert!(
        request.contains(&format!("authorization: Bearer {VALUE}")),
        "{request}"
    );
    let body = request.split("\r\n\r\n").nth(1).expect("тело запроса");
    let json: serde_json::Value = serde_json::from_str(body).expect("тело — JSON");
    assert_eq!(json["model"], MODEL);
    assert_eq!(json["messages"][0]["role"], "user");
    assert_eq!(json["messages"][0]["content"], "ping");
    assert_eq!(json["stream"], false);

    assert!(matches!(&frames[0], Frame::Text { text } if text == "pong"));
    assert_eq!(frames.len(), 1);
}

/// П. 2: `tool_calls` становятся кадрами с идентификатором — по нему гейт и Pi связывают
/// вызов с ответом. Аргументы разбираются из строки: неразобранные не станут `{}` молча.
#[test]
fn tool_calls_become_frames_and_their_arguments_are_parsed() {
    let served = serve(200, |_| {
        r#"{"choices":[{"message":{"content":null,"tool_calls":[
            {"id":"call_1","function":{"name":"bash","arguments":"{\"cmd\":\"touch x\"}"}},
            {"id":"call_2","function":{"name":"read","arguments":"{\"path\":\"a\"}"}}]}}]}"#
            .to_owned()
    });
    let frames = backend(&served.url).turn(&[user("go")]).expect("ход");
    served.worker.join().expect("сервер завершился");

    assert_eq!(frames.len(), 2);
    match (&frames[0], &frames[1]) {
        (
            Frame::ToolCall {
                id: first,
                name: one,
                input: input_one,
            },
            Frame::ToolCall {
                id: second,
                name: two,
                input: input_two,
            },
        ) => {
            assert_eq!(first, "call_1");
            assert_eq!(one, "bash");
            assert_eq!(input_one["cmd"], "touch x");
            assert_eq!(second, "call_2");
            assert_eq!(two, "read");
            assert_eq!(input_two["path"], "a");
        }
        other => panic!("два вызова инструментов, а не {other:?}"),
    }
}

/// П. 3: не-2xx — отказ с кодом. Тело ответа к этому моменту уже смыто, поэтому значение
/// токена в сообщении отсутствует даже когда сервис отразил заголовок.
#[test]
fn a_failed_status_carries_the_code_and_never_the_token() {
    let served = serve(401, |request| format!("{{\"echo\":\"{request}\"}}"));
    let refused = backend(&served.url)
        .turn(&[user("ping")])
        .expect_err("не-2xx — отказ");
    served.worker.join().expect("сервер завершился");
    let shown = format!("{}{refused:?}", refused.message);
    assert!(shown.contains("401"), "{shown}");
    assert!(!shown.contains(VALUE), "{shown}");
}

/// П. 4: неразобранный ответ — отказ, а не пустой ход: иначе «сервис ответил мусором»
/// выглядело бы как «модель промолчала».
#[test]
fn a_malformed_response_is_an_error_not_an_empty_turn() {
    let served = serve(200, |_| "not json".to_owned());
    let refused = backend(&served.url)
        .turn(&[user("ping")])
        .expect_err("мусор — отказ");
    served.worker.join().expect("сервер завершился");
    assert!(refused.message.contains("json"), "{}", refused.message);
}

/// П. 5: пустой список `choices` — отсутствие данных, а не завершённый ход. Закон 6.
#[test]
fn empty_choices_are_an_error_not_a_finished_turn() {
    let served = serve(200, |_| r#"{"choices":[]}"#.to_owned());
    let refused = backend(&served.url)
        .turn(&[user("ping")])
        .expect_err("пустой выбор — отказ");
    served.worker.join().expect("сервер завершился");
    assert!(refused.message.contains("choices"), "{}", refused.message);
}

/// Тело запроса как JSON: заголовки отделены пустой строкой, разбор один для всех тестов.
fn body(request: &str) -> serde_json::Value {
    let raw = request.split("\r\n\r\n").nth(1).expect("тело запроса");
    serde_json::from_str(raw).expect("тело — JSON")
}

/// П. 6: непустой набор уезжает в тело как `tools`, и определение не теряет схему.
#[test]
fn the_request_offers_the_turn_tools() {
    let served = serve(200, |_| {
        r#"{"choices":[{"message":{"content":"ok"}}]}"#.to_owned()
    });
    let request = TurnRequest {
        tools: Some(vec![ToolDef {
            name: "bash".to_owned(),
            description: "run a shell command".to_owned(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {"cmd": {"type": "string"}},
                "required": ["cmd"],
            }),
        }]),
        messages: vec![user("go")],
    };
    let frames = backend(&served.url)
        .turn_request(&request)
        .expect("ход с инструментами");
    let raw = served.worker.join().expect("сервер завершился");
    let json = body(&raw);

    let tools = json["tools"].as_array().expect("tools — массив");
    assert_eq!(tools.len(), 1);
    let function = &tools[0]["function"];
    assert_eq!(function["name"], "bash");
    assert_eq!(function["description"], "run a shell command");
    assert_eq!(function["parameters"]["type"], "object");
    assert_eq!(
        function["parameters"]["properties"]["cmd"]["type"],
        "string"
    );
    assert!(matches!(&frames[0], Frame::Text { text } if text == "ok"));
}

/// П. 7: `Some([])` — сознательное «инструментов нет», а не `"tools":[]`: ключа нет.
#[test]
fn an_empty_tool_set_sends_no_tools_key() {
    let served = serve(200, |_| {
        r#"{"choices":[{"message":{"content":"ok"}}]}"#.to_owned()
    });
    let request = TurnRequest {
        tools: Some(Vec::new()),
        messages: vec![user("go")],
    };
    backend(&served.url)
        .turn_request(&request)
        .expect("ход без инструментов");
    let raw = served.worker.join().expect("сервер завершился");
    assert!(body(&raw).get("tools").is_none(), "{raw}");
}

/// П. 8: legacy `None` не порождает ключ `tools` — набор неизвестен, а не пуст.
#[test]
fn a_legacy_turn_sends_no_tools_key() {
    let served = serve(200, |_| {
        r#"{"choices":[{"message":{"content":"ok"}}]}"#.to_owned()
    });
    let request = TurnRequest {
        tools: None,
        messages: vec![user("go")],
    };
    backend(&served.url)
        .turn_request(&request)
        .expect("legacy-ход");
    let raw = served.worker.join().expect("сервер завершился");
    assert!(body(&raw).get("tools").is_none(), "{raw}");
}

/// П. 9: каждый инструмент — ровно обёртка `function` с тремя полями, порядок сохранён.
#[test]
fn tools_render_as_function_definitions() {
    let served = serve(200, |_| {
        r#"{"choices":[{"message":{"content":"ok"}}]}"#.to_owned()
    });
    let request = TurnRequest {
        tools: Some(vec![
            ToolDef {
                name: "read".to_owned(),
                description: "read a file".to_owned(),
                parameters: serde_json::json!({"type": "object", "properties": {}}),
            },
            ToolDef {
                name: "write".to_owned(),
                description: "write a file".to_owned(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {"path": {"type": "string"}},
                }),
            },
        ]),
        messages: vec![user("go")],
    };
    backend(&served.url)
        .turn_request(&request)
        .expect("ход с двумя инструментами");
    let raw = served.worker.join().expect("сервер завершился");
    let json = body(&raw);
    let tools = json["tools"].as_array().expect("tools — массив");

    assert_eq!(tools.len(), 2);
    let names: Vec<&str> = tools
        .iter()
        .map(|tool| tool["function"]["name"].as_str().expect("имя строкой"))
        .collect();
    assert_eq!(names, ["read", "write"]);
    for tool in tools {
        let wrapper = tool.as_object().expect("обёртка — объект");
        assert_eq!(wrapper.len(), 2);
        assert_eq!(wrapper["type"], "function");
        let function = wrapper["function"].as_object().expect("function — объект");
        assert_eq!(function.len(), 3);
        assert!(function.contains_key("name"));
        assert!(function.contains_key("description"));
        assert!(function.contains_key("parameters"));
    }
    assert_eq!(tools[0]["function"]["description"], "read a file");
    assert_eq!(
        tools[1]["function"]["parameters"],
        serde_json::json!({"type": "object", "properties": {"path": {"type": "string"}}})
    );
}

fn chat_ok() -> Served {
    serve(200, |_| {
        r#"{"choices":[{"message":{"content":"ok"}}]}"#.to_owned()
    })
}

fn sent(served: Served, messages: &[TurnMessage]) -> serde_json::Value {
    backend(&served.url).turn(messages).expect("ход");
    body(&served.worker.join().expect("сервер завершился"))
}

fn call(id: &str, name: &str, arguments: serde_json::Value) -> AssistantCall {
    AssistantCall {
        id: id.to_owned(),
        name: name.to_owned(),
        arguments,
    }
}

fn assistant(text: Option<&str>, tool_calls: Vec<AssistantCall>) -> TurnMessage {
    TurnMessage::Assistant {
        text: text.map(str::to_owned),
        tool_calls,
    }
}

fn tool(tool_call_id: &str, name: &str, text: &str, is_error: bool) -> TurnMessage {
    TurnMessage::Tool {
        tool_call_id: tool_call_id.to_owned(),
        name: name.to_owned(),
        text: text.to_owned(),
        is_error,
    }
}

/// П. 10: результат инструмента уходит тем же `tool_call_id`, что и породивший его вызов.
#[test]
fn a_tool_result_keeps_the_call_id_it_answers() {
    let messages = vec![
        user("go"),
        assistant(
            None,
            vec![call("call_1", "bash", serde_json::json!({"cmd": "true"}))],
        ),
        tool("call_1", "bash", "done", false),
    ];
    let json = sent(chat_ok(), &messages);
    assert_eq!(json["messages"][2]["role"], "tool");
    assert_eq!(json["messages"][2]["tool_call_id"], "call_1");
    assert_eq!(json["messages"][2]["content"], "done");
    assert_eq!(json["messages"][1]["tool_calls"][0]["id"], "call_1");
}

/// П. 11: assistant-сообщение несёт сделанные вызовы в порядке хода, а не теряет их.
#[test]
fn an_assistant_turn_carries_the_calls_it_made() {
    let messages = vec![
        user("go"),
        assistant(
            Some("делаю"),
            vec![
                call("call_1", "bash", serde_json::json!({"cmd": "true"})),
                call("call_2", "read", serde_json::json!({"path": "a"})),
            ],
        ),
    ];
    let json = sent(chat_ok(), &messages);
    let calls = json["messages"][1]["tool_calls"]
        .as_array()
        .expect("tool_calls — массив");
    assert_eq!(json["messages"][1]["content"], "делаю");
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0]["id"], "call_1");
    assert_eq!(calls[0]["function"]["name"], "bash");
    assert_eq!(calls[1]["id"], "call_2");
    assert_eq!(calls[1]["function"]["name"], "read");
}

/// П. 12: system-сообщение остаётся system, а не превращается в реплику пользователя.
#[test]
fn a_system_message_stays_a_system_message() {
    let messages = vec![
        TurnMessage::System {
            text: "правила".to_owned(),
        },
        user("go"),
    ];
    let json = sent(chat_ok(), &messages);
    assert_eq!(json["messages"][0]["role"], "system");
    assert_eq!(json["messages"][0]["content"], "правила");
    assert_eq!(json["messages"][1]["role"], "user");
}

/// П. 13: tool-сообщение — роль, `tool_call_id`, контент; `name` не уезжает, ошибка — маркером.
#[test]
fn tool_results_render_as_openai_tool_messages() {
    let messages = vec![
        tool("call_ok", "bash", "вывод", false),
        tool("call_err", "bash", "boom", true),
    ];
    let json = sent(chat_ok(), &messages);
    assert_eq!(json["messages"][0]["role"], "tool");
    assert_eq!(json["messages"][0]["tool_call_id"], "call_ok");
    assert_eq!(json["messages"][0]["content"], "вывод");
    assert_eq!(
        json["messages"][1]["content"],
        "<tool_error>boom</tool_error>"
    );
    assert!(json["messages"][0].get("name").is_none());
    assert!(json["messages"][1].get("name").is_none());
}

#[test]
fn an_assistant_tool_call_renders_arguments_as_a_string() {
    let messages = vec![assistant(
        None,
        vec![call(
            "call_1",
            "bash",
            serde_json::json!({"cmd": "touch x"}),
        )],
    )];
    let json = sent(chat_ok(), &messages);
    let rendered = &json["messages"][0]["tool_calls"][0];
    assert_eq!(rendered["type"], "function");
    let arguments = rendered["function"]["arguments"]
        .as_str()
        .expect("arguments — строка, а не JSON-объект");
    let parsed: serde_json::Value = serde_json::from_str(arguments).expect("строка — JSON");
    assert_eq!(parsed, serde_json::json!({"cmd": "touch x"}));
}

/// Backend, который запоминает разговор: тесты разбора идут через настоящий `parse`.
struct Capturing(Mutex<Vec<TurnMessage>>);

impl ModelBackend for Capturing {
    fn turn(&self, messages: &[TurnMessage]) -> Result<Vec<Frame>, BackendError> {
        *self.0.lock().expect("журнал сообщений") = messages.to_vec();
        Ok(vec![Frame::Text {
            text: "ok".to_owned(),
        }])
    }
}

fn turn(messages: &str) -> String {
    format!("{{\"kind\":\"turn\",\"tools\":[],\"messages\":{messages}}}\n")
}

fn read_until_terminal(stream: &UnixStream) -> Vec<Frame> {
    let mut frames = Vec::new();
    for line in BufReader::new(stream.try_clone().expect("клон сокета")).lines() {
        let frame: Frame =
            serde_json::from_str(&line.expect("ответ читается")).expect("кадр — JSON");
        let terminal = matches!(frame, Frame::Done { .. } | Frame::Error { .. });
        frames.push(frame);
        if terminal {
            break;
        }
    }
    frames
}

/// Одна заявка через настоящий сокет: разбор идёт `ModelProxy`-ем, а не конструктором enum.
fn parse_through_proxy(tag: &str, payload: &str) -> (Vec<Frame>, Vec<TurnMessage>) {
    let dir = std::env::temp_dir().join(format!("sw-n2-00j5-{tag}-{}", std::process::id()));
    let proxy = Arc::new(ModelProxy::bind(&dir).expect("прокси слушает сокет"));
    let backend = Arc::new(Capturing(Mutex::new(Vec::new())));
    let worker = {
        let proxy = Arc::clone(&proxy);
        let backend = Arc::clone(&backend);
        std::thread::spawn(move || proxy.serve_once(backend.as_ref()))
    };
    let mut stream = UnixStream::connect(proxy.socket()).expect("клиент подключается");
    stream
        .write_all(payload.as_bytes())
        .expect("полезное записано");
    let frames = read_until_terminal(&stream);
    drop(stream);
    worker
        .join()
        .expect("поток сервера")
        .expect("прокси не упал");
    let seen = backend.0.lock().expect("журнал сообщений").clone();
    (frames, seen)
}

fn refusal_code(frames: &[Frame]) -> ErrorCode {
    match frames {
        [Frame::Error { code, .. }] => *code,
        other => panic!("ожидался ровно один error-кадр, а не {other:?}"),
    }
}

#[test]
fn a_tool_message_without_a_call_id_is_refused() {
    let payloads = [
        turn(r#"[{"role":"tool","name":"bash","text":"done","is_error":false}]"#),
        turn(r#"[{"role":"tool","tool_call_id":"","name":"bash","text":"done","is_error":false}]"#),
    ];
    for (index, payload) in payloads.into_iter().enumerate() {
        let (frames, seen) = parse_through_proxy(&format!("tool-no-id-{index}"), &payload);
        assert_eq!(refusal_code(&frames), ErrorCode::Schema, "{payload}");
        assert!(seen.is_empty(), "до backend'а не дошло: {seen:?}");
    }
}

/// П. 16: assistant без текста и без вызовов — не ход, а нарушение схемы.
#[test]
fn an_assistant_with_neither_text_nor_calls_is_refused() {
    let (frames, seen) = parse_through_proxy("assistant-empty", &turn(r#"[{"role":"assistant"}]"#));
    assert_eq!(refusal_code(&frames), ErrorCode::Schema);
    assert!(seen.is_empty(), "до backend'а не дошло: {seen:?}");
}

/// П. 17: неизвестный ключ сообщения отвергается, а не игнорируется молча.
#[test]
fn an_unknown_message_key_is_refused() {
    let (frames, seen) = parse_through_proxy(
        "unknown-message-key",
        &turn(r#"[{"role":"system","text":"x","image":"y"}]"#),
    );
    assert_eq!(refusal_code(&frames), ErrorCode::Schema);
    assert!(seen.is_empty(), "до backend'а не дошло: {seen:?}");
}

/// П. 18: отсутствие `tool_calls` у assistant — это пустой список, а не «данных нет».
#[test]
fn tool_calls_default_to_empty() {
    let (frames, seen) = parse_through_proxy(
        "calls-default",
        &turn(r#"[{"role":"assistant","text":"ok"}]"#),
    );
    assert!(
        matches!(frames.last(), Some(Frame::Done { .. })),
        "разговор дошёл до backend'а: {frames:?}"
    );
    match seen.as_slice() {
        [TurnMessage::Assistant { text, tool_calls }] => {
            assert_eq!(text.as_deref(), Some("ok"));
            assert!(tool_calls.is_empty(), "вызовы пусты: {tool_calls:?}");
        }
        other => panic!("ожидался один assistant, а не {other:?}"),
    }
}
