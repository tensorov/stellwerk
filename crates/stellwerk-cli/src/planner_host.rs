//! # Канал планировщика
//!
//! Планирование ничего не исполняет: модель возвращает текст. Поэтому здесь нет ни
//! песочницы, ни гейта — они нужны там, где есть эффект (Build), а не там, где есть только
//! слова. Сам ход идёт тем же путём, что и в Build: заявка в `model.sock`, прокси подставляет
//! токен, backend говорит с сервисом.

use std::fmt;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;

use crate::host_model::{HostModel, HostModelError, ModelConfig};

/// Потолок одного хода. Не «на всякий случай»: без него молчащий сервис держал бы TUI
/// вечно, а человек не отличил бы «модель думает» от «канал умер».
const TURN_TIMEOUT: Duration = Duration::from_secs(120);

/// Почему плана нет. Шесть состояний, потому что «отказ сервиса», «пустой ответ» и
/// «инструмент там, где их не давали» — разные вещи и требуют разных действий.
#[derive(Debug)]
pub enum PlannerError {
    /// Настройка или прокси не собрались.
    Model(HostModelError),
    /// Сокет не открылся.
    Channel(String),
    /// Сервис ответил отказом.
    Refused { code: String, message: String },
    /// Модель запросила инструмент. Планировщик их не давал, значит маршрут не тот.
    UnexpectedTool,
}

impl fmt::Display for PlannerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Model(why) => write!(f, "{why}"),
            Self::Channel(why) => write!(f, "model channel: {why}"),
            Self::Refused { code, message } => {
                write!(f, "service refused ({code}): {message}")
            }
            Self::UnexpectedTool => {
                write!(f, "model asked for a tool the planner never offered")
            }
        }
    }
}

impl std::error::Error for PlannerError {}

/// Планировщик: один вопрос — один ход.
pub struct PlannerHost {
    model: HostModel,
}

impl PlannerHost {
    pub fn prepare(
        config: ModelConfig,
        scratch: &Path,
        visible: &[PathBuf],
    ) -> Result<Self, PlannerError> {
        Ok(Self {
            model: HostModel::prepare(config, scratch, visible).map_err(PlannerError::Model)?,
        })
    }

    pub fn socket(&self) -> &Path {
        self.model.socket()
    }

    /// Порядок: сначала соединение, потом прокси. Если сокета нет, отказ приходит здесь и
    /// поток обслуживания не поднимается — иначе он остался бы висеть в `accept`.
    pub fn ask(&self, prompt: &str) -> Result<String, PlannerError> {
        let stream = UnixStream::connect(self.model.socket())
            .map_err(|why| PlannerError::Channel(format!("connect: {why}")))?;
        stream
            .set_read_timeout(Some(TURN_TIMEOUT))
            .map_err(|why| PlannerError::Channel(format!("timeout: {why}")))?;
        // Прокси обслуживает одно соединение за вызов и блокируется, поэтому ход идёт
        // навстречу из отдельного потока. `scope` дожидается его до возврата: `done`
        // записан прежде, чем вызывающий увидит текст.
        std::thread::scope(|scope| {
            scope.spawn(|| {
                let _ = self.model.serve_once();
            });
            let mut writer = stream
                .try_clone()
                .map_err(|why| PlannerError::Channel(format!("clone: {why}")))?;
            let request = serde_json::json!({
                "kind": "turn",
                "messages": [{ "role": "user", "text": prompt }],
            });
            writer
                .write_all(format!("{request}\n").as_bytes())
                .map_err(|why| PlannerError::Channel(format!("write: {why}")))?;
            writer
                .flush()
                .map_err(|why| PlannerError::Channel(format!("flush: {why}")))?;
            read_reply(stream)
        })
    }
}

fn read_reply(stream: UnixStream) -> Result<String, PlannerError> {
    let mut text = String::new();
    for line in BufReader::new(stream).lines() {
        let line = line.map_err(|why| PlannerError::Channel(format!("read: {why}")))?;
        let frame: Value = serde_json::from_str(&line)
            .map_err(|why| PlannerError::Channel(format!("frame: {why}")))?;
        match frame.get("kind").and_then(Value::as_str) {
            Some("text") => {
                if let Some(part) = frame.get("text").and_then(Value::as_str) {
                    text.push_str(part);
                }
            }
            Some("tool_call") => return Err(PlannerError::UnexpectedTool),
            Some("error") => {
                let named = |key: &str| {
                    frame
                        .get(key)
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                        .ok_or_else(|| {
                            PlannerError::Channel(format!("error frame without `{key}`"))
                        })
                };
                return Err(PlannerError::Refused {
                    code: named("code")?,
                    message: named("message")?,
                });
            }
            Some("done") => break,
            _ => {}
        }
    }
    // Сюда доходит только ответ, оборванный без отказа: backend пустой ход не пропускает.
    // Это дефект протокола, а не свойство модели, поэтому `Channel`, а не «пустой план».
    if text.trim().is_empty() {
        return Err(PlannerError::Channel(
            "answer ended without text and without an error".to_owned(),
        ));
    }
    Ok(text)
}
