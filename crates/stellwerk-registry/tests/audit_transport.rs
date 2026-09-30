//! Audit transport driven over in-memory bytes: `initialize` → `tools/list` → `tools/call`
//! with no child process, no tempdir and no new dependency.
//!
//! The transport is generic over `<R: BufRead, W: Write>` precisely so this file can feed
//! canned server lines through it — the same code path `StdioProbe`/`ChildStdout` uses in
//! production, minus the process.

use std::io::{self, BufRead, Cursor, Read};
use std::sync::mpsc;

use serde_json::json;
use stellwerk_registry::cmd_audit::{
    audit_manifest, AuditError, CallOutcome, McpProbe, Rpc, RpcAnswer,
};

/// Тестовый бюджет: 50 мс вместо прод-10-секунд, чтобы таймаут был наблюдаем.
const TEST_BUDGET_MS: u64 = 50;

const INITIALIZE_RESULT: &str = r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","capabilities":{},"serverInfo":{"name":"fake","version":"0"}}}"#;
const INITIALIZED_NOTIFICATION: &str = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;

/// Ответ `tools/list` на заданный `id`: MCP требует `inputSchema` у каждого тулла.
fn tools_list(id: u64, tools: &[&str]) -> String {
    let items: Vec<String> = tools
        .iter()
        .map(|name| format!(r#"{{"name":"{name}","inputSchema":{{}}}}"#))
        .collect();
    format!(
        r#"{{"jsonrpc":"2.0","id":{id},"result":{{"tools":[{}]}}}}"#,
        items.join(",")
    )
}

/// Собрать поток сервера: одна строка JSON = одно сообщение newline-delimited протокола.
fn stream(lines: &[&str]) -> Vec<u8> {
    let mut out = String::new();
    for line in lines {
        out.push_str(line);
        out.push('\n');
    }
    out.into_bytes()
}

/// Сервер, который не отвечает никогда: единственный `read` блокируется, пока тест жив.
/// Ни процесса, ни файла — ровно то состояние, которое обязан пережить `recv_timeout`.
struct SilentServer {
    /// Sender намеренно не в структуре: его держит тест, иначе канал закрылся бы и
    /// `recv` вернул бы Err вместо блокировки.
    never: mpsc::Receiver<()>,
}

impl SilentServer {
    /// Простоять до тех пор, пока тест не отпустит канал: сервер молчит бесконечно.
    fn park(&mut self) -> io::Result<()> {
        self.never
            .recv()
            .map_err(|_| io::Error::other("test released the silence"))
    }
}

impl Read for SilentServer {
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        self.park()?;
        Ok(0)
    }
}

impl BufRead for SilentServer {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.park()?;
        Ok(&[])
    }

    fn consume(&mut self, _amount: usize) {}
}

#[test]
fn full_cycle_over_memory_reports_no_discrepancies() {
    let server = stream(&[
        INITIALIZE_RESULT,
        INITIALIZED_NOTIFICATION,
        &tools_list(2, &["t1", "t2"]),
        r#"{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"ok"}]}}"#,
        r#"{"jsonrpc":"2.0","id":4,"result":{"content":[]}}"#,
        r#"{"jsonrpc":"2.0","id":5,"result":{"content":[{"type":"text","text":"ok"}]}}"#,
    ]);
    let mut written: Vec<u8> = Vec::new();
    let mut rpc = Rpc::new(Cursor::new(server), &mut written, TEST_BUDGET_MS);

    rpc.handshake().expect("handshake");
    let tools = vec!["t1".to_owned(), "t2".to_owned()];
    let report = audit_manifest(&tools, &mut rpc, TEST_BUDGET_MS).expect("must report");

    assert!(
        report.missing_in_server.is_empty(),
        "{:?}",
        report.missing_in_server
    );
    assert!(
        report.extra_in_server.is_empty(),
        "{:?}",
        report.extra_in_server
    );
    assert!(
        report.not_invocable.is_empty(),
        "{:?}",
        report.not_invocable
    );

    let outcome = rpc.call_tool("t1", "{}").expect("call");
    assert!(matches!(outcome, CallOutcome::Result(_)), "got {outcome:?}");

    let sent = String::from_utf8(written).expect("utf8 request log");
    for method in [
        "initialize",
        "notifications/initialized",
        "tools/list",
        "tools/call",
    ] {
        assert!(
            sent.contains(method),
            "handshake must send {method}: {sent}"
        );
    }
}

#[test]
fn jsonrpc_error_on_call_keeps_the_tool_invocable() {
    // Q3: invalid-args проба отбивается ошибкой, а не молчанием — тул жив.
    let first = r#"{"jsonrpc":"2.0","id":3,"error":{"code":-32602,"message":"invalid arguments"}}"#;
    let second = first.replace(r#""id":3"#, r#""id":4"#);
    let server = stream(&[
        INITIALIZE_RESULT,
        INITIALIZED_NOTIFICATION,
        &tools_list(2, &["t1"]),
        first,
        &second,
    ]);
    let mut rpc = Rpc::new(Cursor::new(server), Vec::<u8>::new(), TEST_BUDGET_MS);

    rpc.handshake().expect("handshake");
    let tools = vec!["t1".to_owned()];
    let report = audit_manifest(&tools, &mut rpc, TEST_BUDGET_MS).expect("must report");
    assert!(
        report.not_invocable.is_empty(),
        "{:?}",
        report.not_invocable
    );

    let outcome = rpc.call_tool("t1", "{}").expect("call");
    match outcome {
        CallOutcome::Error(detail) => assert_eq!(detail["code"], -32602),
        other => panic!("expected CallOutcome::Error, got {other:?}"),
    }
}

#[test]
fn unparseable_answer_on_call_is_malformed() {
    let server = stream(&[
        INITIALIZE_RESULT,
        INITIALIZED_NOTIFICATION,
        &tools_list(2, &["t1"]),
        "this is not json at all",
    ]);
    let mut rpc = Rpc::new(Cursor::new(server), Vec::<u8>::new(), TEST_BUDGET_MS);

    rpc.handshake().expect("handshake");
    let tools = vec!["t1".to_owned()];
    let report = audit_manifest(&tools, &mut rpc, TEST_BUDGET_MS).expect("must report");
    assert_eq!(report.not_invocable, vec!["t1".to_owned()]);
}

#[test]
fn answer_with_neither_result_nor_error_is_malformed() {
    let server = stream(&[
        INITIALIZE_RESULT,
        INITIALIZED_NOTIFICATION,
        r#"{"jsonrpc":"2.0","id":3,"jsonrpc":"2.0"}"#,
    ]);
    let mut rpc = Rpc::new(Cursor::new(server), Vec::<u8>::new(), TEST_BUDGET_MS);

    rpc.handshake().expect("handshake");
    let outcome = rpc.call_tool("t1", "{}").expect("call");
    assert!(matches!(outcome, CallOutcome::Malformed), "got {outcome:?}");
}

#[test]
fn silent_server_is_a_timeout_under_the_injected_budget() {
    // Два зонда: после таймаута читатель остаётся в своём потоке, и следующий запрос
    // уже не может безопасно спарить ответы — сессия недоступна, а не «ещё раз ждём».
    let (_keepalive, never) = mpsc::channel::<()>();
    let mut rpc = Rpc::new(SilentServer { never }, Vec::<u8>::new(), TEST_BUDGET_MS);
    let answer = rpc
        .request("tools/list", &json!({}))
        .expect("silence is not an error");
    assert!(matches!(answer, RpcAnswer::TimedOut), "got {answer:?}");

    let (_keepalive, never) = mpsc::channel::<()>();
    let mut rpc = Rpc::new(SilentServer { never }, Vec::<u8>::new(), TEST_BUDGET_MS);
    let outcome = rpc.call_tool("t1", "{}").expect("call");
    assert!(matches!(outcome, CallOutcome::Timeout), "got {outcome:?}");
}

#[test]
fn notifications_do_not_derail_the_answer_for_an_id() {
    let list_changed = r#"{"jsonrpc":"2.0","method":"notifications/tools/list_changed"}"#;
    let server = stream(&[
        INITIALIZE_RESULT,
        INITIALIZED_NOTIFICATION,
        list_changed,
        &tools_list(2, &["t1", "t2"]),
    ]);
    let mut rpc = Rpc::new(Cursor::new(server), Vec::<u8>::new(), TEST_BUDGET_MS);

    rpc.handshake().expect("handshake");
    let names = rpc.list_tools().expect("list");
    assert_eq!(names, vec!["t1".to_owned(), "t2".to_owned()]);
}

#[test]
fn chatty_server_is_cut_off_instead_of_spinning() {
    let list = tools_list(2, &["t1"]);
    let mut lines = vec![INITIALIZE_RESULT, INITIALIZED_NOTIFICATION];
    lines.extend(std::iter::repeat_n(INITIALIZED_NOTIFICATION, 70));
    lines.push(&list);
    let mut rpc = Rpc::new(
        Cursor::new(stream(&lines)),
        Vec::<u8>::new(),
        TEST_BUDGET_MS,
    );

    rpc.handshake().expect("handshake");
    let err = rpc
        .list_tools()
        .expect_err("70 notifications must be cut off");
    assert!(matches!(err, AuditError::NoAnswer(_)), "got {err:?}");
}

#[test]
fn tools_list_without_a_result_is_not_an_empty_list() {
    let server = stream(&[
        INITIALIZE_RESULT,
        INITIALIZED_NOTIFICATION,
        r#"{"jsonrpc":"2.0","id":2,"result":{}}"#,
    ]);
    let mut rpc = Rpc::new(Cursor::new(server), Vec::<u8>::new(), TEST_BUDGET_MS);

    rpc.handshake().expect("handshake");
    let err = rpc
        .list_tools()
        .expect_err("a result without tools is not an empty tool list");
    assert!(matches!(err, AuditError::NoAnswer(_)), "got {err:?}");
}

#[test]
fn zero_budget_is_refused_rather_than_read_as_a_timeout() {
    let mut rpc = Rpc::new(Cursor::new(Vec::<u8>::new()), Vec::<u8>::new(), 0);
    let err = audit_manifest(&[], &mut rpc, 0).expect_err("no budget is not a clean audit");
    assert!(matches!(err, AuditError::NoBudget), "got {err:?}");
}
