//! # Запуск воркера и разбор потока Pi
//!
//! Раннер — граница, где чужой процесс становится событиями федерации: Pi стартует
//! ВНУТРИ песочницы (`command_in` отдаёт непрочитанную команду, чтобы отказ программы
//! не слился с отказом песочницы), а его stdout читается построчно.
//!
//! Поток — НЕдоверенный ввод, и строка может оказаться не-JSON. Молча пропустить её
//! нельзя: прогон, потерявший шаг, неотличим от прогона без шага (закон 6).
//!
//! RED M2/packet-09: тела — `unimplemented!()` (реализация — M2/packet-10).

use crate::sandbox::Sandbox;
use stellwerk_core::contract::{ModelBinding, TaskSpec};
use thiserror::Error;

/// Отказ раннера: `Spawn` (нечего запускать) и `MalformedLine` (строка не-JSON) — разные причины.
#[derive(Debug, Error)]
pub enum RunnerError {
    #[error("worker `{program}` did not start: {source}")]
    Spawn {
        program: String,
        source: std::io::Error,
    },
    /// Номер и текст обязательны: «битая строка где-то» не расследуется, а молчаливый
    /// пропуск строки — это потерянный шаг, неотличимый от прогона без шага.
    #[error("line {line_no} of the worker stream is not a JSON event: {line:?}")]
    MalformedLine { line_no: u32, line: String },
}

#[derive(Debug)]
pub struct RunRequest {
    pub spec: TaskSpec,
    pub model: ModelBinding,
    pub sandbox: Sandbox,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunOutcome {
    pub stop_reason: StopReason,
    pub tool_steps: u32,
    pub turns: u32,
    /// Входные токены вместе с кэшем — три счётчика `validity_gate.py:59`.
    pub total_input_tokens: u64,
    pub verdict: Verdict,
}

/// Почему воркер остановился. `Error`/`Timeout`/`Aborted` — не то же, что `Stop`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// Модель дошла до конца задания.
    Stop,
    /// Провайдер или рантайм сбойнулся.
    Error,
    /// Прогон не уложился в отведённое время.
    Timeout,
    /// Прогон прерван внешним требованием.
    Aborted,
}

/// Вердикт гейта: годен прогон или нет — и ПОЧЕМУ нет. Не `gate::Verdict` из gateway:
/// там разрешение вызова, здесь — качество прогона.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Valid,
    /// Остановка не прошла `GATE_REQUIRE_STOP_OK`: прогона как такового нет.
    InvalidLoop,
    /// Ни одного шага инструмента: модель только говорила, не действовала.
    InvalidNoToolSteps,
    /// Ходов меньше порога `GATE_REQUIRE_TURNS`: подозрительно короткий прогон.
    InvalidTooFewTurns,
}

/// Накопленный разбор потока — счётчики, из которых собирается `RunOutcome`. `stop_reason`
/// — `Option`: «провайдер не назвал причину» не значит ни `Stop`, ни `Error` (закон 6).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Progress {
    pub tool_steps: u32,
    pub turns: u32,
    pub total_input_tokens: u64,
    pub stop_reason: Option<StopReason>,
}

impl Progress {
    pub fn absorb(&mut self, line: &str) -> Result<(), RunnerError> {
        let _ = line;
        unimplemented!()
    }

    /// Прочитать поток до конца. Источник подменяемый: тест не зовёт настоящий Pi.
    pub fn drain(&mut self, source: &mut impl EventSource) -> Result<(), RunnerError> {
        let _ = source;
        unimplemented!()
    }
}

pub trait EventSource {
    fn next_line(&mut self) -> Option<String>;
}

/// Вердикт по накопленному прогону. Пороги — из `scripts/worker/dispatch.conf:73–75`
/// (`GATE_REQUIRE_STOP_OK`, `GATE_REQUIRE_TOOL_STEPS`, `GATE_REQUIRE_TURNS`); умолчания
/// 1/1/1 значат: остановка чистая, шаг есть, ходов больше одного.
pub fn verdict_for(stop: StopReason, tool_steps: u32, turns: u32) -> Verdict {
    let _ = (stop, tool_steps, turns);
    unimplemented!()
}
