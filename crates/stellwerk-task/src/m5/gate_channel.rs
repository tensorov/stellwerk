//! # Канал авторизации инструмента
//!
//! Песочница спрашивает хоста ДО эффекта: «можно ли вызвать `tool` из этой сессии?».
//! Классификацию риска и решение оставляет хост: в кадре заявки нет ни `risk`, ни
//! `module`, ни `caller`, ни `data`, поэтому песочница не может выписать себе разрешение.
//!
//! Каждый ответ — отдельное состояние. `deny` (запрещено политикой), `suspended`
//! (спросить было некого), `unclassified` (о риске нет данных), `unknown_tool` и
//! `unrecognised_source` (вызов не опознан) не сводятся ни друг к другу, ни к `allow`:
//! «никто не ответил» не должно читаться как «можно».

use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use stellwerk_gateway::gate::{decide, CallRequest, Verdict};

use super::model_transport::MAX_FRAME_BYTES;
use super::session_identity::{Adjudication, IdentityError, SessionDescription};

/// Заявка песочницы. Ровно три ключа: риск и полномочия хост берёт из своей политики.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateRequest {
    pub kind: String,
    pub tool: String,
    pub parent_tool_call_id: Option<String>,
    /// Путь, на который собирается действовать инструмент (`event.input.path` в Pi).
    /// `None` — прислали без пути; для пишущего инструмента это отсутствие данных,
    /// а не «путь не нужен».
    #[serde(default)]
    pub path: Option<String>,
}

/// Ответ хоста. Один кадр на соединение, терминальный.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GateAnswer {
    Allow {
        tool: String,
    },
    /// Таблица разрешила, и у разрешения есть названное основание: путь совпал с
    /// записью утверждённого плана. Отдельное состояние, а не `Allow`: «разрешено
    /// таблицей» и «разрешено планом владельца» — разные факты.
    Authorized {
        tool: String,
        scope: String,
    },
    Ask {
        tool: String,
    },
    Suspended {
        tool: String,
    },
    Deny {
        tool: String,
        reason: String,
    },
    UnknownTool {
        tool: String,
        held: Vec<String>,
    },
    UnrecognisedSource {
        parent_tool_call_id: String,
    },
    Unclassified {
        tool: String,
    },
    Refused {
        code: RefusalCode,
        message: String,
    },
}

/// Код отказа разбора или проверки прав. Клиент различает состояния по нему.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefusalCode {
    UnknownKind,
    MalformedJson,
    UnexpectedField,
    Schema,
    OversizeFrame,
    Identity,
}

/// Основание для `Allow`. Называется явно, чтобы разрешение записи нельзя было
/// перепутать с разрешением чтения.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllowBasis {
    /// Так решила таблица рисков (`permission_for`).
    RiskTable,
    /// Путь совпал с записью одобренного плана; `scope` — эта запись дословно.
    ApprovedPlan { scope: String },
}

/// Что политика говорит о вызове. `Refused` — названный отказ политики (путь вне
/// одобренного плана), и он не имеет права выглядеть как `Unclassified`: «запрещено»
/// и «нет данных» — разные состояния.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyVerdict {
    Evaluated {
        request: CallRequest,
        basis: AllowBasis,
    },
    Unclassified,
    Refused {
        reason: String,
    },
}

/// Хостовая политика: единственное место, где известен риск инструмента. `None` — данных
/// о риске нет, и это отдельное состояние, а не «риска ноль».
pub trait GatePolicy: Send + Sync {
    fn request_for(&self, call: &Adjudication) -> Option<CallRequest>;

    /// Решение с аргументом вызова. Умолчание отказывает, если путь пришёл, а политика
    /// его не толкует: пропустить неразобранный путь значило бы разрешить запись,
    /// о которой никто не высказался.
    fn decide_call(&self, call: &Adjudication, path: Option<&str>) -> PolicyVerdict {
        if path.is_some() {
            return PolicyVerdict::Refused {
                reason: "policy_does_not_handle_paths".to_owned(),
            };
        }
        match self.request_for(call) {
            Some(request) => PolicyVerdict::Evaluated {
                request,
                basis: AllowBasis::RiskTable,
            },
            None => PolicyVerdict::Unclassified,
        }
    }
}

/// Итог обслуживания соединения: три состояния, а не «успех/нет».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateServed {
    Answered,
    NoRequest,
    Refused,
}

/// Хостовый канал: слушает `gate.sock` в хозяйском каталоге и удаляет файл при уничтожении.
pub struct GateChannel {
    listener: UnixListener,
    socket: PathBuf,
}

impl GateChannel {
    pub fn bind(dir: &Path) -> io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let socket = dir.join("gate.sock");
        let listener = UnixListener::bind(&socket)?;
        Ok(Self { listener, socket })
    }

    pub fn socket(&self) -> &Path {
        &self.socket
    }

    /// Одно соединение — один вердикт. Вторая заявка не обслуживается.
    pub fn serve_once(
        &self,
        session: &SessionDescription,
        policy: &dyn GatePolicy,
    ) -> io::Result<GateServed> {
        self.serve_once_logged(session, policy)
            .map(|(served, _answer)| served)
    }

    /// То же, но с ответом: вызывающий ведёт журнал решений, и «сколько заявок»
    /// без самих решений ничего не доказывает.
    pub fn serve_once_logged(
        &self,
        session: &SessionDescription,
        policy: &dyn GatePolicy,
    ) -> io::Result<(GateServed, Option<GateAnswer>)> {
        let (stream, _address) = self.listener.accept()?;
        let mut writer = stream.try_clone()?;
        let mut reader = BufReader::new(stream);
        let answer = match read_request(&mut reader)? {
            ReadOutcome::Eof => return Ok((GateServed::NoRequest, None)),
            ReadOutcome::Refused(code, message) => GateAnswer::Refused { code, message },
            ReadOutcome::Request(request) => answer_for(session, policy, &request),
        };
        write_answer(&mut writer, &answer)?;
        // Хвост дочитываем до EOF: закрытие с непрочитанными байтами шлёт RST, и клиент
        // не увидит сам ответ. Предел — тот же, что у кадра.
        let tail = &mut reader.by_ref().take(MAX_FRAME_BYTES as u64);
        io::copy(tail, &mut io::sink())?;
        let served = match answer {
            GateAnswer::Refused { .. } => GateServed::Refused,
            _ => GateServed::Answered,
        };
        Ok((served, Some(answer)))
    }
}

impl Drop for GateChannel {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.socket);
    }
}

enum ReadOutcome {
    Request(GateRequest),
    Eof,
    Refused(RefusalCode, String),
}

fn answer_for(
    session: &SessionDescription,
    policy: &dyn GatePolicy,
    request: &GateRequest,
) -> GateAnswer {
    let call = match session.adjudicate(&request.tool, request.parent_tool_call_id.as_deref()) {
        Ok(call) => call,
        Err(IdentityError::NoSuchTool { tool, held }) => {
            return GateAnswer::UnknownTool { tool, held }
        }
        Err(IdentityError::UnrecognisedSource {
            parent_tool_call_id,
        }) => {
            return GateAnswer::UnrecognisedSource {
                parent_tool_call_id,
            }
        }
        Err(other) => {
            return GateAnswer::Refused {
                code: RefusalCode::Identity,
                message: other.to_string(),
            }
        }
    };
    let tool = call.tool.clone();
    match policy.decide_call(&call, request.path.as_deref()) {
        PolicyVerdict::Unclassified => GateAnswer::Unclassified { tool },
        PolicyVerdict::Refused { reason } => GateAnswer::Deny { tool, reason },
        PolicyVerdict::Evaluated { request, basis } => match decide(&request) {
            // База только аннотирует разрешение таблицы и никогда его не подменяет:
            // если таблица сказала Ask или Deny, основание не превращает это в Allow.
            Verdict::Allow => match basis {
                AllowBasis::RiskTable => GateAnswer::Allow { tool },
                AllowBasis::ApprovedPlan { scope } => GateAnswer::Authorized { tool, scope },
            },
            Verdict::Ask => GateAnswer::Ask { tool },
            Verdict::Suspended => GateAnswer::Suspended { tool },
            Verdict::Deny { reason } => GateAnswer::Deny { tool, reason },
        },
    }
}

fn read_request(reader: &mut BufReader<UnixStream>) -> io::Result<ReadOutcome> {
    let mut frame = Vec::new();
    let read = reader
        .by_ref()
        .take(MAX_FRAME_BYTES as u64 + 1)
        .read_until(b'\n', &mut frame)?;
    if read == 0 {
        return Ok(ReadOutcome::Eof);
    }
    if frame.len() > MAX_FRAME_BYTES {
        return Ok(ReadOutcome::Refused(
            RefusalCode::OversizeFrame,
            format!("frame exceeds {MAX_FRAME_BYTES} bytes"),
        ));
    }
    if !frame.ends_with(b"\n") {
        return Ok(ReadOutcome::Refused(
            RefusalCode::MalformedJson,
            "frame ended before its newline".to_owned(),
        ));
    }
    frame.pop();
    let text = match String::from_utf8(frame) {
        Ok(text) => text,
        Err(_) => {
            return Ok(ReadOutcome::Refused(
                RefusalCode::MalformedJson,
                "frame is not UTF-8".to_owned(),
            ))
        }
    };
    Ok(match parse_request(&text) {
        Ok(request) => ReadOutcome::Request(request),
        Err((code, message)) => ReadOutcome::Refused(code, message),
    })
}

fn parse_request(text: &str) -> Result<GateRequest, (RefusalCode, String)> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|error| {
        (
            RefusalCode::MalformedJson,
            format!("not valid JSON: {error}"),
        )
    })?;
    let Some(object) = value.as_object() else {
        return Err((
            RefusalCode::MalformedJson,
            "frame is not a JSON object".to_owned(),
        ));
    };
    let extra = object.keys().find(|key| {
        !matches!(
            key.as_str(),
            "kind" | "tool" | "parent_tool_call_id" | "path"
        )
    });
    if let Some(field) = extra {
        return Err((
            RefusalCode::UnexpectedField,
            format!("unexpected field `{field}`"),
        ));
    }
    let kind = object
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| (RefusalCode::Schema, "frame has no string `kind`".to_owned()))?;
    if kind != "tool_call" {
        return Err((RefusalCode::UnknownKind, format!("unknown kind `{kind}`")));
    }
    serde_json::from_str(text).map_err(|error| (RefusalCode::Schema, error.to_string()))
}

fn write_answer(writer: &mut UnixStream, answer: &GateAnswer) -> io::Result<()> {
    let line = serde_json::to_vec(answer).map_err(io::Error::other)?;
    writer.write_all(&[line.as_slice(), b"\n"].concat())?;
    Ok(())
}
