//! Audit: сверка манифеста с живым сервером — шов packet-08, сверка в 09, транспорт в 09b.
//!
//! Шов транспортно-независим и остаётся sync (`&mut self`): M0 не тянет async-клиент,
//! а тесты не должны поднимать живой MCP-процесс. Транспорт обобщён над `<R: BufRead, W: Write>`,
//! поэтому один и тот же код гоняется и на `&[u8]` в памяти, и на `ChildStdout` сервера.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use rmcp::model::{ListToolsResult, ProtocolVersion};
use serde_json::{json, Value};

use crate::model::{Module, ModuleKind};

/// Прод-бюджет на один тул; тесты инжектят 50 мс (packet-08, Контекст).
pub const AUDIT_TIMEOUT_MS: u64 = 10_000;

#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    /// Заглушка RED-пакета: реальный stdio/rmcp транспорт приходит в packet-09b.
    #[error("audit not implemented (packet-09b)")]
    NotImplemented,
    /// У модуля нет `command` — это не то же самое, что «сервер не поднялся».
    #[error("module {0} has no command")]
    NoCommand(String),
    #[error("failed to spawn MCP server")]
    Spawn,
    /// Сессия не дала пригодного ответа; причина обязана быть названа, иначе «сервер не
    /// поднялся» и «сервер не ответил» становятся одним и тем же диагнозом.
    #[error("mcp session gave no usable answer: {0}")]
    NoAnswer(&'static str),
    /// `args` — не JSON-документ: это дефект вызывающего, а не ответ сервера.
    #[error("tool arguments are not a JSON document")]
    BadArguments,
    /// Бюджет не задан. Ноль — не «без ограничения» и не «всё по таймауту»: отсутствие
    /// бюджета обязано быть видно, иначе молчаливый отказ неотличим от отказа сервера.
    #[error("audit timeout budget is zero")]
    NoBudget,
    #[error("audit io: {0}")]
    Io(#[from] std::io::Error),
}

/// Транспортно-независимый шов аудита.
pub trait McpProbe {
    /// `tools/list` — имена ровно те, что отдаёт сервер.
    fn list_tools(&mut self) -> Result<Vec<String>, AuditError>;
    /// `tools/call` с `{}` как invalid-args пробой (Q3): валидный JSON-RPC ответ сам по
    /// себе доказывает вызываемость — валидацию аргументов делает сервер.
    fn call_tool(&mut self, name: &str, args: &str) -> Result<CallOutcome, AuditError>;
}

/// Исход одного `tools/call`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallOutcome {
    Result(serde_json::Value),
    /// JSON-RPC error — тул жив (Q3), в not_invocable не попадает.
    Error(serde_json::Value),
    Timeout,
    Malformed,
}

/// Отчёт по одному модулю; пустые векторы = «всё совпало» (exit 0).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditReport {
    /// Заявлено в манифесте, нет на сервере.
    pub missing_in_server: Vec<String>,
    /// Есть на сервере, манифестом не заявлено.
    pub extra_in_server: Vec<String>,
    /// Заявлен, присутствует, но не вызывается (таймаут / битый ответ).
    pub not_invocable: Vec<String>,
    /// `kind = skill`: аудит отложен, spawn не выполнялся.
    pub skill_skipped: bool,
}

/// Сверка exact-set имён + вызываемости каждого заявленного тулла.
///
/// `timeout_ms` — тот же бюджет, что пробник получил при сборке: нулевой бюджет
/// отвергается, потому что «бюджета нет» и «всё по таймауту» — разные диагнозы.
pub fn audit_manifest(
    tools: &[String],
    probe: &mut dyn McpProbe,
    timeout_ms: u64,
) -> Result<AuditReport, AuditError> {
    if timeout_ms == 0 {
        return Err(AuditError::NoBudget);
    }
    let server_tools = probe.list_tools()?;

    let missing_in_server: Vec<String> = tools
        .iter()
        .filter(|name| !server_tools.contains(name))
        .cloned()
        .collect();
    let extra_in_server: Vec<String> = server_tools
        .iter()
        .filter(|name| !tools.contains(name))
        .cloned()
        .collect();

    let mut not_invocable = Vec::new();
    for name in tools.iter().filter(|name| server_tools.contains(*name)) {
        // Проба невалидными аргументами (Q3): валидный JSON-RPC ответ сам по себе
        // доказывает вызываемость, валидацию аргументов делает сервер.
        let outcome = probe.call_tool(name, "{}")?;
        if matches!(outcome, CallOutcome::Timeout | CallOutcome::Malformed) {
            not_invocable.push(name.clone());
        }
    }

    Ok(AuditReport {
        missing_in_server,
        extra_in_server,
        not_invocable,
        // Этот шов обслуживает только `kind = mcp`; пропуск skill выставляет audit_module.
        skill_skipped: false,
    })
}

// ── transport ───────────────────────────────────────────────────────────────

/// Что дал один обмен. Три состояния не взаимозаменяемы: конверт сервера, истёкший
/// бюджет и нечитаемый поток различимы и в статусе, и в evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcAnswer {
    /// Конверт на запрошенный `id` — с `result` или с `error`.
    Answered(Value),
    /// Бюджет истёк: сервер молчит. Это не «сервер не ответил» — это «мы перестали ждать».
    TimedOut,
    /// Поток закрылся, отдал не-JSON или упёрся в границу прочитанных сообщений:
    /// кадрирование сломано, вызываемость не доказана.
    Unreadable,
}

fn write_message<W: Write>(writer: &mut W, line: &str) -> Result<(), AuditError> {
    writer.write_all(line.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

/// Построчно читать до конверта на `id`; уведомления пропускаются.
///
/// `MAX_SKIPPED` ограничивает число прочитанных сообщений: MCP stdio — это
/// newline-delimited JSON, и сервер со сломанным кадрированием должен быть отрезан,
/// а не крутить вызывающего вечно (прецедент — `cmd_measure::read_response`).
fn read_response<R: BufRead>(reader: &mut R, id: u64) -> RpcAnswer {
    const MAX_SKIPPED: usize = 64;
    let mut line = String::new();
    for _ in 0..MAX_SKIPPED {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => return RpcAnswer::Unreadable,
            Ok(_) => {}
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(trimmed) else {
            return RpcAnswer::Unreadable;
        };
        if value.get("id").and_then(Value::as_u64) == Some(id) {
            return RpcAnswer::Answered(value);
        }
    }
    RpcAnswer::Unreadable
}

/// Конверт, на который можно опереться: таймаут, не-JSON и пустой поток — не «ответ».
fn answered(answer: RpcAnswer) -> Result<Value, AuditError> {
    match answer {
        RpcAnswer::Answered(value) => Ok(value),
        RpcAnswer::TimedOut => Err(AuditError::NoAnswer("budget expired before the answer")),
        RpcAnswer::Unreadable => Err(AuditError::NoAnswer("closed stream, non-JSON, bad framing")),
    }
}

/// Синхронный JSON-RPC транспорт аудита поверх произвольной пары reader/writer.
///
/// Обобщение `<R: BufRead, W: Write>` — то же, что у `cmd_measure::run_session` и по той
/// же причине: `tests/audit_transport.rs` гоняет полный цикл на `&[u8]` в памяти, без
/// процесса, а прод собирает ту же структуру на `ChildStdout`/`ChildStdin`.
pub struct Rpc<R: BufRead, W: Write> {
    /// `None` только на время, пока читатель живёт в своём потоке ожидания.
    reader: Option<R>,
    writer: W,
    next_id: u64,
    timeout_ms: u64,
    /// Сервер, породивший эту пару: без него аудит оставил бы зомби на каждый запуск.
    child: ChildGuard,
}

impl<R: BufRead + Send + 'static, W: Write> Rpc<R, W> {
    pub fn new(reader: R, writer: W, timeout_ms: u64) -> Self {
        Self {
            reader: Some(reader),
            writer,
            next_id: 1,
            timeout_ms,
            child: ChildGuard(None),
        }
    }

    fn with_child(mut self, child: Child) -> Self {
        self.child = ChildGuard(Some(child));
        self
    }

    /// Рукопожатие MCP: `initialize`, затем `notifications/initialized`.
    ///
    /// Ответ с `error` здесь — провал рукопожатия, а не «ответ без результата»: продолжить
    /// сессию после него нельзя, и молча притвориться, что всё хорошо, было бы враньём.
    pub fn handshake(&mut self) -> Result<(), AuditError> {
        let params = json!({
            "protocolVersion": ProtocolVersion::V_2024_11_05.as_str(),
            "capabilities": {},
            "clientInfo": {"name": "stellwerk", "version": env!("CARGO_PKG_VERSION")},
        });
        let init = answered(self.request("initialize", &params)?)?;
        if init.get("result").is_none() {
            return Err(AuditError::NoAnswer(
                "initialize returned an error envelope",
            ));
        }
        write_message(
            &mut self.writer,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        )
    }

    /// Один обмен: записать запрос и ждать ответа на свой `id` не дольше бюджета.
    ///
    /// Ожидание живёт в отдельном потоке, а бюджет в нём — инжектированное число.
    /// После таймаута читатель остаётся в том потоке: поток уже не сочтётся со своими
    /// запросами, поэтому следующий обмен падает явно, вместо угадывания, чей это ответ.
    pub fn request(&mut self, method: &str, params: &Value) -> Result<RpcAnswer, AuditError> {
        let id = self.next_id;
        self.next_id += 1;
        let line = json!({"jsonrpc":"2.0","id":id,"method":method,"params":params});
        write_message(&mut self.writer, &line.to_string())?;
        let Some(mut reader) = self.reader.take() else {
            return Err(AuditError::NoAnswer("reader is parked after a timeout"));
        };
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let answer = read_response(&mut reader, id);
            let _ = tx.send((answer, reader));
        });
        match rx.recv_timeout(Duration::from_millis(self.timeout_ms)) {
            Err(RecvTimeoutError::Timeout) => Ok(RpcAnswer::TimedOut),
            Err(RecvTimeoutError::Disconnected) => Err(AuditError::NoAnswer("reader died")),
            Ok((answer, reader)) => {
                self.reader = Some(reader);
                Ok(answer)
            }
        }
    }
}

impl Rpc<BufReader<ChildStdout>, ChildStdin> {
    /// Живой транспорт: поднять `command` модуля и говорить с ним по stdio.
    pub fn spawn(command: &str, timeout_ms: u64) -> Result<Self, AuditError> {
        let mut child = Command::new(command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => AuditError::Spawn,
                _ => AuditError::Io(e),
            })?;
        let (stdin, stdout) = child
            .stdin
            .take()
            .zip(child.stdout.take())
            .ok_or(AuditError::NoAnswer("child pipes unavailable"))?;
        Ok(Self::new(BufReader::new(stdout), stdin, timeout_ms).with_child(child))
    }

    fn shutdown(&mut self) {
        self.child.reap();
    }
}

struct ChildGuard(Option<Child>);

impl ChildGuard {
    /// Завершить процесс: `kill`, затем `wait`. Именно `wait` убирает запись из таблицы
    /// процессов, а один только `kill` оставил бы зомби до конца жизни родителя.
    ///
    /// `kill`, а не EOF: `writer` нельзя освободить из `drop`, а `wait` до его освобождения встал
    /// бы в deadlock — сервер ждал бы EOF, родитель ждал бы сервер. `Child::kill` в std есть,
    /// поэтому reap остаётся ограниченным по времени и зависший сервер не подвесит аудит.
    fn reap(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for ChildGuard {
    /// Гарантия на каждом раннем выходе, включая `?` в вызывающем коде: `Child` в std не делает
    /// ни `kill`, ни `wait` при drop, поэтому без этого зомби жил бы до конца процесса.
    fn drop(&mut self) {
        self.reap();
    }
}

impl<R: BufRead + Send + 'static, W: Write> McpProbe for Rpc<R, W> {
    fn list_tools(&mut self) -> Result<Vec<String>, AuditError> {
        let envelope = answered(self.request("tools/list", &json!({}))?)?;
        // Конверт без `result` — это не пустой список тулов: разные состояния обязаны
        // иметь разные диагнозы, иначе «сервер молчит» станется «тулов нет».
        let result = envelope
            .get("result")
            .cloned()
            .ok_or(AuditError::NoAnswer("tools/list carries no result"))?;
        let list: ListToolsResult = serde_json::from_value(result)
            .map_err(|_| AuditError::NoAnswer("tools/list is not a tools list"))?;
        Ok(list
            .tools
            .into_iter()
            .map(|tool| tool.name.into_owned())
            .collect())
    }

    fn call_tool(&mut self, name: &str, args: &str) -> Result<CallOutcome, AuditError> {
        let arguments: Value = serde_json::from_str(args).map_err(|_| AuditError::BadArguments)?;
        let params = json!({"name": name, "arguments": arguments});
        let envelope = match self.request("tools/call", &params)? {
            RpcAnswer::TimedOut => return Ok(CallOutcome::Timeout),
            RpcAnswer::Unreadable => return Ok(CallOutcome::Malformed),
            RpcAnswer::Answered(envelope) => envelope,
        };
        Ok(match (envelope.get("result"), envelope.get("error")) {
            (Some(result), _) => CallOutcome::Result(result.clone()),
            (_, Some(error)) => CallOutcome::Error(error.clone()),
            // Ни `result`, ни `error`: ответ есть, вызываемость не доказана (Q3).
            (None, None) => CallOutcome::Malformed,
        })
    }
}

// ── module-level audit ──────────────────────────────────────────────────────

/// Аудит одного модуля: `kind = skill` → пропуск, `kind = mcp` → spawn + сверка.
///
/// Матч по `ModuleKind` полный: новый вид модуля не должен молча уйти из-под аудита.
pub fn audit_module(
    module: &Module,
    _root: &Path,
    timeout_ms: u64,
) -> Result<AuditReport, AuditError> {
    match module.kind {
        ModuleKind::Skill => Ok(AuditReport {
            missing_in_server: vec![],
            extra_in_server: vec![],
            not_invocable: vec![],
            skill_skipped: true,
        }),
        ModuleKind::Mcp => {
            // Имени модуля в сигнатуре нет, а `NoCommand` его требует: идентифицируем
            // модуль источником — единственным идентификатором, который даёт `Module`.
            let command = module
                .command
                .as_deref()
                .ok_or_else(|| AuditError::NoCommand(format!("{:?}", module.source)))?;
            let mut probe = Rpc::spawn(command, timeout_ms)?;
            probe.handshake()?;
            let report = audit_manifest(&module.tools, &mut probe, timeout_ms)?;
            probe.shutdown();
            Ok(report)
        }
    }
}

// ── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ModulesFile;
    use serde_json::json;

    /// Тестовый бюджет: 50 мс вместо прод-10-секунд, чтобы таймаут был наблюдаем.
    const TEST_TIMEOUT_MS: u64 = 50;

    /// Зонд-фейк: сервер отдаёт заданный список имён, на `tools/call` — заданный исход
    /// (по умолчанию успешный result). Часов в шове нет: таймаут — это исход, а не ожидание.
    struct FakeProbe {
        server_tools: Vec<String>,
        outcomes: Vec<(&'static str, CallOutcome)>,
    }

    impl FakeProbe {
        fn new(server_tools: &[&str]) -> Self {
            Self {
                server_tools: server_tools.iter().map(|s| (*s).to_owned()).collect(),
                outcomes: Vec::new(),
            }
        }

        fn outcome(mut self, name: &'static str, outcome: CallOutcome) -> Self {
            self.outcomes.push((name, outcome));
            self
        }
    }

    impl McpProbe for FakeProbe {
        fn list_tools(&mut self) -> Result<Vec<String>, AuditError> {
            Ok(self.server_tools.clone())
        }

        fn call_tool(&mut self, name: &str, _args: &str) -> Result<CallOutcome, AuditError> {
            match self.outcomes.iter().find(|(n, _)| *n == name) {
                Some((_, outcome)) => Ok(outcome.clone()),
                None => Ok(CallOutcome::Result(json!({}))),
            }
        }
    }

    fn audit(manifest: &[&str], mut probe: FakeProbe) -> Result<AuditReport, AuditError> {
        let tools: Vec<String> = manifest.iter().map(|s| (*s).to_owned()).collect();
        audit_manifest(&tools, &mut probe, TEST_TIMEOUT_MS)
    }

    #[test]
    fn audit_exact_match_reports_no_failures() {
        let r = audit(&["a", "b", "c"], FakeProbe::new(&["a", "b", "c"])).expect("must report");
        assert!(r.missing_in_server.is_empty() && r.extra_in_server.is_empty());
        assert!(r.not_invocable.is_empty());
        assert!(!r.skill_skipped, "mcp-аудит не пропускается");
    }

    #[test]
    fn audit_missing_in_server_names_manifest_only_tool() {
        let r = audit(&["a", "b", "c"], FakeProbe::new(&["a", "b"])).expect("must report");
        assert_eq!(r.missing_in_server, vec!["c".to_owned()]);
    }

    #[test]
    fn audit_extra_in_server_names_server_only_tool() {
        let r =
            audit(&["a", "b", "c"], FakeProbe::new(&["a", "b", "c", "d"])).expect("must report");
        assert_eq!(r.extra_in_server, vec!["d".to_owned()]);
    }

    #[test]
    fn audit_not_invocable_on_timeout() {
        let p = FakeProbe::new(&["a", "b", "c"]).outcome("c", CallOutcome::Timeout);
        let r = audit(&["a", "b", "c"], p).expect("must report");
        assert_eq!(r.not_invocable, vec!["c".to_owned()]);
    }

    #[test]
    fn audit_not_invocable_on_malformed_answer() {
        let p = FakeProbe::new(&["a", "b", "c"]).outcome("b", CallOutcome::Malformed);
        let r = audit(&["a", "b", "c"], p).expect("must report");
        assert_eq!(r.not_invocable, vec!["b".to_owned()]);
    }

    #[test]
    fn audit_jsonrpc_error_is_still_invocable() {
        // Q3: invalid-args проба отбивается ошибкой, а не молчанием — тул жив.
        let p = FakeProbe::new(&["a"]).outcome("a", CallOutcome::Error(json!({"code": -32602})));
        let r = audit(&["a"], p).expect("must report");
        assert!(r.not_invocable.is_empty(), "JSON-RPC error = вызываем");
    }

    const SKILL_TOML: &str = r#"
[registry.calibration]
tokens_per_byte = 0.5
measured_at = "2026-01-01T00:00:00Z"

[module.notes]
kind = "skill"
source = { npm = "@scope/notes", version = "1.0.0" }
"#;

    #[test]
    fn audit_skill_module_is_skipped_without_spawn() {
        let mf = ModulesFile::from_toml_str(SKILL_TOML).expect("fixture must parse");
        let r = audit_module(&mf.module["notes"], Path::new("."), TEST_TIMEOUT_MS)
            .expect("skill is deferred, not an error");
        assert!(r.skill_skipped, "skill audit deferred");
        assert!(r.missing_in_server.is_empty() && r.not_invocable.is_empty());
    }

    #[test]
    fn audit_timeout_budget_is_injected_not_hardcoded() {
        // 50 мс в тесте и 10_000 мс в проде — разные числа: зашитый в шов дефолт
        // тихо утопил бы тестовый бюджет, поэтому оба значения зафиксированы.
        assert_eq!(AUDIT_TIMEOUT_MS, 10_000, "прод-дефолт зафиксирован пакетом");
        let r = audit(&["a"], FakeProbe::new(&["a"])).expect("must report under injected budget");
        assert!(r.not_invocable.is_empty());
    }

    #[test]
    fn audit_empty_manifest_and_empty_server_pass_vacuously() {
        let r = audit(&[], FakeProbe::new(&[])).expect("nothing declared, nothing to fail");
        assert!(r.missing_in_server.is_empty() && r.extra_in_server.is_empty());
        assert!(r.not_invocable.is_empty() && !r.skill_skipped);
    }
}
