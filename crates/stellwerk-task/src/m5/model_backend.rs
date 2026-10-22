//! # Backend модели: ход уходит в OpenAI-совместимый сервис с хоста
//!
//! Песочница не имеет сети, поэтому говорит только с `model.sock`; в интернет ходит хост.
//! Токен здесь не читается и не печатается: его подставляет `TokenProxy`, а backend видит
//! лишь готовый ответ — так значение остаётся в одном месте.

use serde::Deserialize;

use super::http_wire::BaseUrl;
use super::model_transport::{BackendError, Frame, ModelBackend, Role, TurnMessage};
use super::secrets::{ProxyRequest, TokenProxy};

/// Ходовой backend: адрес и модель заданы при сборке, токен живёт в прокси.
#[derive(Debug, Clone)]
pub struct HttpBackend {
    base: BaseUrl,
    model: String,
    token: TokenProxy,
}

impl HttpBackend {
    /// Сборка проверяет адрес сразу: неверная схема или открытый HTTP вне loopback — отказ
    /// до первого хода, а не после того, как токен уже ушёл.
    pub fn new(base_url: &str, model_id: &str, token: TokenProxy) -> Result<Self, BackendError> {
        Ok(Self {
            base: BaseUrl::parse(base_url).map_err(refused)?,
            model: model_id.to_owned(),
            token,
        })
    }
}

impl ModelBackend for HttpBackend {
    fn turn(&self, messages: &[TurnMessage]) -> Result<Vec<Frame>, BackendError> {
        let turns: Vec<serde_json::Value> = messages
            .iter()
            .map(|message| {
                serde_json::json!({
                    "role": match message.role {
                        Role::User => "user",
                        Role::Assistant => "assistant",
                    },
                    "content": message.text,
                })
            })
            .collect();
        let body = serde_json::json!({
            "model": self.model,
            "messages": turns,
            "stream": false,
        })
        .to_string();
        let (authority, path) = (self.base.authority(), self.base.path("/chat/completions"));
        let request = ProxyRequest::new(
            "POST",
            &authority,
            &path,
            Some("application/json"),
            Some(&body),
        )
        .map_err(|why| refused(format!("request refused: {why}")))?;
        let response = self
            .token
            .send(request, |wire| self.base.exchange(wire))
            .map_err(|why| refused(format!("proxy refused: {why}")))?;
        if !(200..300).contains(&response.status) {
            return Err(refused(format!(
                "service answered {}: {}",
                response.status, response.body
            )));
        }
        parse(&response.body)
    }
}

/// Разбор ответа: текст и/или вызовы. Ни одного кадра — отказ, а не пустой ход: прокси
/// добавил бы `done`, и «модель промолчала» стало бы неотличимо от «сервис не ответил».
fn parse(body: &str) -> Result<Vec<Frame>, BackendError> {
    #[derive(Deserialize)]
    struct Response {
        choices: Vec<Choice>,
    }
    #[derive(Deserialize)]
    struct Choice {
        message: Message,
    }
    #[derive(Deserialize)]
    struct Message {
        content: Option<String>,
        #[serde(default)]
        tool_calls: Vec<Call>,
    }
    #[derive(Deserialize)]
    struct Call {
        id: String,
        function: Function,
    }
    #[derive(Deserialize)]
    struct Function {
        name: String,
        arguments: String,
    }

    let response: Response = serde_json::from_str(body)
        .map_err(|why| refused(format!("response is not json: {why}")))?;
    let choice = response
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| refused("response carries no choices"))?;
    let mut frames = Vec::new();
    if let Some(text) = choice.message.content.filter(|text| !text.is_empty()) {
        frames.push(Frame::Text { text });
    }
    for call in choice.message.tool_calls {
        let input = serde_json::from_str(&call.function.arguments).map_err(|why| {
            refused(format!(
                "arguments of `{}` are not json: {why}",
                call.function.name
            ))
        })?;
        frames.push(Frame::ToolCall {
            id: call.id,
            name: call.function.name,
            input,
        });
    }
    if frames.is_empty() {
        return Err(refused(
            "response carries neither text nor tool calls: an empty turn is not an answer",
        ));
    }
    Ok(frames)
}

fn refused(message: impl Into<String>) -> BackendError {
    BackendError {
        message: message.into(),
    }
}
