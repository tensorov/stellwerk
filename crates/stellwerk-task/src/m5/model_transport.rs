//! # Транспорт модели: кадры на хостовом сокете
//!
//! Клиент в песочнице говорит с хостом по AF_UNIX. Маршрут выбирает ХОСТ: в схеме заявки
//! нет ни `url`, ни `headers`, ни `base_url`, ни `model`, и кадр с ними отвергается до
//! вызова backend'а. Одна заявка на соединение: вторая не исполняется, даже если дописана
//! в тот же поток.
//!
//! Отказы различаются кодом, а не текстом: неизвестный `kind`, битый JSON, лишнее поле,
//! нарушение схемы и превышение размера — разные состояния; не-UTF-8 и обрыв кадра приходят
//! как `malformed_json` со своим сообщением. Ни один отказ не выглядит как `done`, и
//! «клиент подключился и ушёл» — это `NoRequest`, а не успех.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// 512 КиБ, общий для model proxy и gate channel: больше — отказ, не дожидаясь
/// конца строки.
pub const MAX_FRAME_BYTES: usize = 512 * 1024;

/// Кадр хоста клиенту. `done` и `error` — разные терминальные состояния.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Frame {
    Text {
        text: String,
    },
    ToolCall {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    Done {
        reason: String,
    },
    Error {
        code: ErrorCode,
        message: String,
    },
}

/// Код отказа. Клиент различает состояния по нему, а не по тексту сообщения.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    UnknownKind,
    MalformedJson,
    UnexpectedField,
    Schema,
    OversizeFrame,
    BackendError,
    ToolNotOffered,
}

/// Сообщение разговора. Форму задаёт роль: assistant несёт текст и сделанные вызовы,
/// tool — идентификатор вызова, который он закрывает. Отсутствие `tool_calls` у assistant
/// означает пустой список, но «ни текста, ни вызовов» — уже не ход.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum TurnMessage {
    System {
        text: String,
    },
    User {
        text: String,
    },
    Assistant {
        text: Option<String>,
        #[serde(default)]
        tool_calls: Vec<AssistantCall>,
    },
    Tool {
        tool_call_id: String,
        name: String,
        text: String,
        is_error: bool,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssistantCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// Определение инструмента, предложенного модели. `parameters` — JSON-схема объекта.
#[derive(Debug, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Заявка на ход: необязательный набор инструментов и сообщения.
///
/// `tools: None` — legacy, набор неизвестен; `Some([])` — сознательное «инструментов нет»;
/// `Some(list)` — предложенный список. Три состояния не сливаются в одно.
#[derive(Debug)]
pub struct TurnRequest {
    pub tools: Option<Vec<ToolDef>>,
    pub messages: Vec<TurnMessage>,
}

/// Ответ backend'а: текстовые дельты и запрошенные вызовы. Терминальный `done` добавляет
/// прокси — backend не решает, закончился ли ход.
pub trait ModelBackend: Send + Sync {
    fn turn(&self, messages: &[TurnMessage]) -> Result<Vec<Frame>, BackendError>;

    /// Ход с явным набором инструментов. Backend без поддержки инструментов отвергает
    /// непустой набор, а `None` и `Some([])` проводит через `turn`.
    fn turn_request(&self, request: &TurnRequest) -> Result<Vec<Frame>, BackendError> {
        match &request.tools {
            Some(tools) if !tools.is_empty() => Err(BackendError {
                message: "backend does not accept tools".to_owned(),
            }),
            _ => self.turn(&request.messages),
        }
    }
}

/// Backend не смог ответить. Отдельно от «ответил пустым ходом».
#[derive(Debug, Error)]
#[error("model backend refused the turn: {message}")]
pub struct BackendError {
    pub message: String,
}

/// Итог обслуживания соединения: три состояния, а не «успех/нет».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Served {
    Turn,
    NoRequest,
    Refused,
}

type Refusal = (ErrorCode, String);

fn refusal(code: ErrorCode, message: impl Into<String>) -> Refusal {
    (code, message.into())
}

/// Хостовый прокси: слушает сокет в хозяйском каталоге и удаляет файл при уничтожении.
pub struct ModelProxy {
    listener: UnixListener,
    socket: PathBuf,
}

impl ModelProxy {
    pub fn bind(dir: &Path) -> io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let socket = dir.join("model.sock");
        let listener = UnixListener::bind(&socket)?;
        Ok(Self { listener, socket })
    }

    pub fn socket(&self) -> &Path {
        &self.socket
    }

    /// Одно соединение — одна заявка. После ответа соединение закрывается.
    pub fn serve_once(&self, backend: &dyn ModelBackend) -> io::Result<Served> {
        let (stream, _address) = self.listener.accept()?;
        let mut writer = stream.try_clone()?;
        let mut reader = BufReader::new(stream);
        let parsed = match read_frame(&mut reader)? {
            ReadOutcome::Eof => return Ok(Served::NoRequest),
            ReadOutcome::Refused(error) => Err(error),
            ReadOutcome::Frame(text) => parse(&text),
        };
        let served = match parsed {
            Err((code, message)) => {
                write_frame(&mut writer, &Frame::Error { code, message })?;
                Served::Refused
            }
            Ok(request) => match backend.turn_request(&request) {
                Ok(frames) => {
                    let known = request.tools.as_ref().map(|tools| {
                        tools
                            .iter()
                            .map(|tool| tool.name.as_str())
                            .collect::<Vec<_>>()
                    });
                    let refused = known.as_ref().and_then(|known| {
                        frames.iter().find_map(|frame| match frame {
                            Frame::ToolCall { name, .. } if !known.contains(&name.as_str()) => {
                                Some(name.clone())
                            }
                            _ => None,
                        })
                    });
                    if let Some(name) = refused {
                        write_frame(
                            &mut writer,
                            &Frame::Error {
                                code: ErrorCode::ToolNotOffered,
                                message: format!("tool `{name}` was not offered"),
                            },
                        )?;
                        Served::Refused
                    } else {
                        let reason = if frames
                            .iter()
                            .any(|frame| matches!(frame, Frame::ToolCall { .. }))
                        {
                            "tool_use"
                        } else {
                            "stop"
                        };
                        for frame in frames {
                            write_frame(&mut writer, &frame)?;
                        }
                        write_frame(
                            &mut writer,
                            &Frame::Done {
                                reason: reason.to_owned(),
                            },
                        )?;
                        Served::Turn
                    }
                }
                Err(error) => {
                    write_frame(
                        &mut writer,
                        &Frame::Error {
                            code: ErrorCode::BackendError,
                            message: error.to_string(),
                        },
                    )?;
                    Served::Refused
                }
            },
        };
        // Хвост кадра дочитываем до EOF: закрытие сокета с непрочитанными байтами шлёт RST,
        // и клиент не увидит сам отказ. Предел — тот же, что у кадра.
        let tail = &mut reader.by_ref().take(MAX_FRAME_BYTES as u64);
        io::copy(tail, &mut io::sink())?;
        Ok(served)
    }
}

impl Drop for ModelProxy {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.socket);
    }
}

enum ReadOutcome {
    Frame(String),
    Eof,
    Refused(Refusal),
}

/// Чтение с пределом: `take` останавливает чтение, не давая буферу уйти за лимит.
fn read_frame(reader: &mut BufReader<UnixStream>) -> io::Result<ReadOutcome> {
    let mut frame = Vec::new();
    let read = reader
        .by_ref()
        .take(MAX_FRAME_BYTES as u64 + 1)
        .read_until(b'\n', &mut frame)?;
    if read == 0 {
        return Ok(ReadOutcome::Eof);
    }
    if frame.len() > MAX_FRAME_BYTES {
        return Ok(ReadOutcome::Refused(refusal(
            ErrorCode::OversizeFrame,
            format!("frame exceeds {MAX_FRAME_BYTES} bytes"),
        )));
    }
    if !frame.ends_with(b"\n") {
        return Ok(ReadOutcome::Refused(refusal(
            ErrorCode::MalformedJson,
            "frame ended before its newline",
        )));
    }
    frame.pop();
    match String::from_utf8(frame) {
        Ok(text) => Ok(ReadOutcome::Frame(text)),
        Err(_) => Ok(ReadOutcome::Refused(refusal(
            ErrorCode::MalformedJson,
            "frame is not UTF-8",
        ))),
    }
}

fn parse(text: &str) -> Result<TurnRequest, Refusal> {
    let value: serde_json::Value = serde_json::from_str(text)
        .map_err(|error| refusal(ErrorCode::MalformedJson, format!("not valid JSON: {error}")))?;
    let Some(object) = value.as_object() else {
        return Err(refusal(ErrorCode::Schema, "frame is not a JSON object"));
    };
    let kind = object
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| refusal(ErrorCode::Schema, "frame has no string `kind`"))?;
    if kind != "turn" {
        return Err(refusal(
            ErrorCode::UnknownKind,
            format!("unknown kind `{kind}`"),
        ));
    }
    let extra = object
        .keys()
        .find(|key| !matches!(key.as_str(), "kind" | "tools" | "messages"));
    if let Some(field) = extra {
        return Err(refusal(
            ErrorCode::UnexpectedField,
            format!("unexpected field `{field}`"),
        ));
    }
    let tools = match object.get("tools") {
        None => {
            return Err(refusal(ErrorCode::Schema, "turn has no `tools` field"));
        }
        Some(serde_json::Value::Array(_)) => {
            let tools: Vec<ToolDef> = serde_json::from_value(object["tools"].clone())
                .map_err(|error| refusal(ErrorCode::Schema, error.to_string()))?;
            for tool in &tools {
                if tool.name.is_empty() {
                    return Err(refusal(
                        ErrorCode::Schema,
                        "tool definition has an empty `name`",
                    ));
                }
                if !tool.parameters.is_object() {
                    return Err(refusal(
                        ErrorCode::Schema,
                        format!("tool `{}` parameters are not an object", tool.name),
                    ));
                }
            }
            Some(tools)
        }
        Some(_) => {
            return Err(refusal(
                ErrorCode::Schema,
                "`tools` is present but not an array",
            ));
        }
    };
    let messages = object
        .get("messages")
        .ok_or_else(|| refusal(ErrorCode::Schema, "frame has no `messages`"))?;
    let raw_messages = messages
        .as_array()
        .ok_or_else(|| refusal(ErrorCode::Schema, "`messages` is not an array"))?;
    for raw in raw_messages {
        validate_message_keys(raw)?;
    }
    let messages: Vec<TurnMessage> = serde_json::from_value(messages.clone())
        .map_err(|error| refusal(ErrorCode::Schema, error.to_string()))?;
    validate_messages(&messages)?;
    Ok(TurnRequest { tools, messages })
}

/// Разбор сырых ключей до десериализации: `serde` молча проглотил бы неизвестный ключ,
/// и лишнее поле стало бы неотличимо от сознательного сообщения.
fn validate_message_keys(raw: &serde_json::Value) -> Result<(), Refusal> {
    let Some(object) = raw.as_object() else {
        return Err(refusal(ErrorCode::Schema, "message is not a JSON object"));
    };
    let role = object
        .get("role")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| refusal(ErrorCode::Schema, "message has no string `role`"))?;
    let (allowed, required): (&[&str], &[&str]) = match role {
        "system" | "user" => (&["role", "text"], &["role", "text"]),
        "assistant" => (&["role", "text", "tool_calls"], &["role"]),
        "tool" => (
            &["role", "tool_call_id", "name", "text", "is_error"],
            &["role", "tool_call_id", "name", "text", "is_error"],
        ),
        other => {
            return Err(refusal(
                ErrorCode::Schema,
                format!("unknown message role `{other}`"),
            ));
        }
    };
    if let Some(key) = object.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(refusal(
            ErrorCode::Schema,
            format!("unexpected key `{key}` in a `{role}` message"),
        ));
    }
    if let Some(key) = required.iter().find(|key| !object.contains_key(**key)) {
        return Err(refusal(
            ErrorCode::Schema,
            format!("`{role}` message has no `{key}`"),
        ));
    }
    Ok(())
}

/// Смысловая проверка уже разобранных сообщений: пустой id вызова или tool-сообщение
/// без адресата делают разговор невоспроизводимым.
fn validate_messages(messages: &[TurnMessage]) -> Result<(), Refusal> {
    for message in messages {
        match message {
            TurnMessage::Assistant { text, tool_calls } => {
                if text.is_none() && tool_calls.is_empty() {
                    return Err(refusal(
                        ErrorCode::Schema,
                        "assistant message carries neither text nor tool calls",
                    ));
                }
                for call in tool_calls {
                    if call.id.is_empty() {
                        return Err(refusal(ErrorCode::Schema, "tool call has an empty `id`"));
                    }
                    if !call.arguments.is_object() {
                        return Err(refusal(
                            ErrorCode::Schema,
                            format!("tool call `{}` arguments are not an object", call.id),
                        ));
                    }
                }
            }
            TurnMessage::Tool { tool_call_id, .. } => {
                if tool_call_id.is_empty() {
                    return Err(refusal(
                        ErrorCode::Schema,
                        "tool message has an empty `tool_call_id`",
                    ));
                }
            }
            TurnMessage::System { .. } | TurnMessage::User { .. } => {}
        }
    }
    Ok(())
}

fn write_frame(writer: &mut UnixStream, frame: &Frame) -> io::Result<()> {
    let line = serde_json::to_vec(frame).map_err(io::Error::other)?;
    writer.write_all(&[line.as_slice(), b"\n"].concat())?;
    Ok(())
}
