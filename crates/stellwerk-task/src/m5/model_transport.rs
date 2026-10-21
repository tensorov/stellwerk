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

/// Предел одного кадра: больше — отказ, не дожидаясь конца строки.
pub const MAX_FRAME_BYTES: usize = 64 * 1024;

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
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TurnMessage {
    pub role: Role,
    pub text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
}

/// Ответ backend'а: текстовые дельты и запрошенные вызовы. Терминальный `done` добавляет
/// прокси — backend не решает, закончился ли ход.
pub trait ModelBackend: Send + Sync {
    fn turn(&self, messages: &[TurnMessage]) -> Result<Vec<Frame>, BackendError>;
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
            Ok(messages) => match backend.turn(&messages) {
                Ok(frames) => {
                    for frame in frames {
                        write_frame(&mut writer, &frame)?;
                    }
                    write_frame(
                        &mut writer,
                        &Frame::Done {
                            reason: "stop".to_owned(),
                        },
                    )?;
                    Served::Turn
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

fn parse(text: &str) -> Result<Vec<TurnMessage>, Refusal> {
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
        .find(|key| !matches!(key.as_str(), "kind" | "messages"));
    if let Some(field) = extra {
        return Err(refusal(
            ErrorCode::UnexpectedField,
            format!("unexpected field `{field}`"),
        ));
    }
    let messages = object
        .get("messages")
        .ok_or_else(|| refusal(ErrorCode::Schema, "frame has no `messages`"))?;
    serde_json::from_value(messages.clone())
        .map_err(|error| refusal(ErrorCode::Schema, error.to_string()))
}

fn write_frame(writer: &mut UnixStream, frame: &Frame) -> io::Result<()> {
    let line = serde_json::to_vec(frame).map_err(io::Error::other)?;
    writer.write_all(&[line.as_slice(), b"\n"].concat())?;
    Ok(())
}
