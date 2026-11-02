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

use stellwerk_gateway::gate::{
    authorize_write, decide, writes_content, AuthorizedCall, CallDecision, CallRequest,
    ContentDigest, RequestedCall, Verdict,
};

use super::model_transport::MAX_FRAME_BYTES;
use super::session_identity::{Adjudication, IdentityError, SessionDescription};

/// Заявка песочницы. Риск и полномочия хост берёт из своей политики, а не из кадра.
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
    /// Полный аргумент вызова. `None` — аргумента не было; для пишущего инструмента
    /// с путём это отсутствие данных, а не «тело пустое».
    #[serde(default)]
    pub argument: Option<String>,
    /// Заявленный SHA-256 содержимого. Хост сверяет его с фактическими байтами
    /// аргумента; `None` для пишущего инструмента с путём — названный отказ.
    #[serde(default)]
    pub content_digest: Option<String>,
}

/// Ответ хоста. Один кадр на соединение, терминальный.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GateAnswer {
    Allow {
        tool: String,
    },
    /// Разрешение на запись с названным основанием: путь из таблицы рисков или
    /// запись утверждённого плана, каждый вместе с digest содержимого. Отдельное
    /// состояние, а не `Allow`: запись разрешает только `AuthorizedCall`.
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

/// Итог решения: ответ в сокет и, если гейт авторизовал запись, сам `AuthorizedCall`.
/// Только `authorized()` даёт право писать; `Allow` и прочие ответы его не несут.
#[derive(Debug, Clone)]
pub struct GateDecision {
    answer: GateAnswer,
    call: Option<AuthorizedCall>,
}

impl GateDecision {
    pub fn answer(&self) -> &GateAnswer {
        &self.answer
    }

    pub fn authorized(&self) -> Option<&AuthorizedCall> {
        self.call.as_ref()
    }

    pub fn into_answer(self) -> GateAnswer {
        self.answer
    }
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
    ) -> io::Result<(GateServed, Option<GateDecision>)> {
        let (stream, _address) = self.listener.accept()?;
        let mut writer = stream.try_clone()?;
        let mut reader = BufReader::new(stream);
        let mut authorized = None;
        let answer = match read_request(&mut reader)? {
            ReadOutcome::Eof => return Ok((GateServed::NoRequest, None)),
            ReadOutcome::Refused(code, message) => GateAnswer::Refused { code, message },
            ReadOutcome::Request(request) => {
                answer_for(session, policy, &request, &mut authorized)
            }
        };
        write_answer(&mut writer, &answer)?;
        // Хвост дочитываем до EOF: закрытие с непрочитанными байтами шлёт RST, и клиент
        // не увидит сам ответ. Предел — тот же, что у кадра.
        let tail = &mut reader.by_ref().take(MAX_FRAME_BYTES as u64);
        io::copy(tail, &mut io::sink())?;
        let decision = GateDecision { answer, call: authorized };
        let served = match decision.answer() {
            GateAnswer::Refused { .. } => GateServed::Refused,
            _ => GateServed::Answered,
        };
        Ok((served, Some(decision)))
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

/// Решение по заявке для хоста: сокет получает `answer()`, а применять запись
/// можно только по `authorized()`. Публичная точка входа не выдаёт `AuthorizedCall`
/// вне гейта: значение появляется лишь после `authorize_write` внутри.
pub fn decide_request(
    session: &SessionDescription,
    policy: &dyn GatePolicy,
    request: &GateRequest,
) -> GateDecision {
    let mut call = None;
    let answer = answer_for(session, policy, request, &mut call);
    GateDecision { answer, call }
}

fn answer_for(
    session: &SessionDescription,
    policy: &dyn GatePolicy,
    request: &GateRequest,
    authorized: &mut Option<AuthorizedCall>,
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
    let requested = match requested_write(request) {
        Ok(requested) => requested,
        Err(reason) => return GateAnswer::Deny { tool, reason },
    };
    match policy.decide_call(&call, request.path.as_deref()) {
        PolicyVerdict::Unclassified => GateAnswer::Unclassified { tool },
        PolicyVerdict::Refused { reason } => GateAnswer::Deny { tool, reason },
        PolicyVerdict::Evaluated {
            request: policy_request,
            basis,
        } => match decide(&policy_request) {
            // База только аннотирует разрешение таблицы и никогда его не подменяет:
            // если таблица сказала Ask или Deny, основание не превращает это в Allow.
            Verdict::Allow => match basis {
                // Таблица рисков разрешает пару (tool, path), а не содержимое:
                // пишущий вызов с путём получает `AuthorizedCall`, связанный с
                // digest, поэтому подмена файла после решения не пройдёт под тем же
                // `scope`. Без digest разрешения на запись нет.
                AllowBasis::RiskTable => match requested.as_ref() {
                    Some(requested) => {
                        let scope = request.path.as_deref().unwrap_or_default();
                        match authorize_write(requested, scope) {
                            CallDecision::Authorized(call) => {
                                let scope = call.scope().to_owned();
                                *authorized = Some(call);
                                GateAnswer::Authorized { tool, scope }
                            }
                            CallDecision::Refused { reason } => GateAnswer::Deny { tool, reason },
                        }
                    }
                    None if writes_content(&request.tool) && request.path.is_some() => {
                        let path = request.path.as_deref().unwrap_or_default();
                        GateAnswer::Deny {
                            tool,
                            reason: format!("write_without_content_digest: `{}` with path `{path}` cannot be authorized without a content digest", request.tool),
                        }
                    }
                    None => GateAnswer::Allow { tool },
                },
                // Разрешение записи выдаётся только через `AuthorizedCall`: он
                // связан с digest проверенного содержимого, поэтому замена файла
                // после авторизации не пройдёт под тем же `scope`.
                AllowBasis::ApprovedPlan { scope } => match requested.as_ref() {
                    Some(requested) => match authorize_write(requested, &scope) {
                        CallDecision::Authorized(call) => {
                            let scope = call.scope().to_owned();
                            *authorized = Some(call);
                            GateAnswer::Authorized { tool, scope }
                        }
                        CallDecision::Refused { reason } => GateAnswer::Deny { tool, reason },
                    },
                    // Пишущий вызов с путём, но без digest, не получает разрешения:
                    // план одобрил путь, а не содержимое, и «нет digest» здесь —
                    // отсутствие данных, а не «файл не менялся».
                    None if writes_content(&request.tool) && request.path.is_some() => {
                        let path = request.path.as_deref().unwrap_or_default();
                        GateAnswer::Deny {
                            tool,
                            reason: format!("write_without_content_digest: `{}` with path `{path}` cannot be authorized without a content digest", request.tool),
                        }
                    }
                    None => GateAnswer::Authorized { tool, scope },
                },
            },
            Verdict::Ask => GateAnswer::Ask { tool },
            Verdict::Suspended => GateAnswer::Suspended { tool },
            Verdict::Deny { reason } => GateAnswer::Deny { tool, reason },
        },
    }
}

/// Проверенный пишущий вызов. Неполный новый контракт (аргумент без digest или
/// digest без аргумента) и digest, не описывающий переданные байты, — названные
/// отказы: «путь есть» не значит «содержимое то же».
///
/// Кадр без аргумента и digest для `edit` — старый `tool_call` Pi, который пока
/// исполняет встроенный инструмент: `Ok(None)` не разрешение, а «typed-запроса
/// не было». Такой вызов всё равно не получит `AuthorizedCall` (см. `answer_for`),
/// а R03 переводит встроенные инструменты на `registerTool` с полным аргументом.
/// `write`/`write_note` без аргумента и digest — отказ: у них нет legacy-пути.
fn requested_write(request: &GateRequest) -> Result<Option<RequestedCall>, String> {
    if !writes_content(&request.tool) {
        return Ok(None);
    }
    let Some(path) = request.path.as_deref() else {
        return Ok(None);
    };
    if request.tool == "edit" && request.argument.is_none() && request.content_digest.is_none() {
        return Ok(None);
    }
    let (Some(argument), Some(claimed)) = (
        request.argument.as_deref(),
        request.content_digest.as_deref(),
    ) else {
        let reason = if request.content_digest.is_none() {
            format!("write_without_content_digest: `{}` with path `{path}` came without a content digest; a path alone does not say what will be written", request.tool)
        } else {
            format!(
                "write_without_argument: `{}` with path `{path}` came without its argument",
                request.tool
            )
        };
        return Err(reason);
    };
    let claimed = ContentDigest::parse(claimed)
        .map_err(|error| format!("malformed_content_digest: {error}"))?;
    RequestedCall::new(
        request.tool.clone(),
        Some(path.to_owned()),
        argument.as_bytes().to_vec(),
        &claimed,
    )
    .map(Some)
    .map_err(|error| format!("content_digest_mismatch: {error}"))
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
            "kind" | "tool" | "parent_tool_call_id" | "path" | "argument" | "content_digest"
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
