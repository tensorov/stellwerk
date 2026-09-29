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
use serde_json::Value;
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
    /// Строк разобрано, с единицы. Поле закрытое: номер битой строки выдаётся вместе с
    /// ней, и подделать его руками нельзя — иначе расследование уйдёт не туда.
    lines_seen: u32,
}

impl Progress {
    /// Разобрать одну строку потока. Пустая строка — не событие. Не-JSON — не «не
    /// событие», а `MalformedLine` с номером и текстом: прогон, потерявший шаг,
    /// неотличим от прогона без шага (закон 6).
    pub fn absorb(&mut self, line: &str) -> Result<(), RunnerError> {
        self.lines_seen += 1;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(());
        }
        let event: Value =
            serde_json::from_str(trimmed).map_err(|_| RunnerError::MalformedLine {
                line_no: self.lines_seen,
                line: line.to_string(),
            })?;
        self.count(&event);
        Ok(())
    }

    /// Прочитать поток до конца. Источник подменяемый: тест не зовёт настоящий Pi.
    /// Нумерация строк — здесь же, в `absorb`: иначе «строка N» означала бы разные
    /// вещи при разборе по одной и при разборе потока.
    pub fn drain(&mut self, source: &mut impl EventSource) -> Result<(), RunnerError> {
        while let Some(line) = source.next_line() {
            self.absorb(&line)?;
        }
        Ok(())
    }

    /// Счётчики из одного события.
    fn count(&mut self, event: &Value) {
        match event.get("type").and_then(Value::as_str) {
            // Шаг — это ход: `toolResults` в потоке пуст, а шаг засчитан (packet-09,
            // тест `each_turn_end_adds_exactly_one_step`).
            Some("turn_end") => {
                self.turns += 1;
                self.tool_steps += 1;
                self.take_stop(event);
            }
            Some("message_update") => self.take_usage(event),
            // Тип, которого мы не моделируем, — не ошибка: формат потока расширяется,
            // и лишнее событие не должно ронять прогон.
            _ => {}
        }
    }

    /// `message.stopReason` — единственный источник «почему остановились». Имя, которого
    /// мы не знаем, — НЕ «нормальная остановка»: такого ответа в модели нет, и подставить
    /// `Stop` значило бы выдумать решение провайдера. Причины нет — остаётся `None`.
    fn take_stop(&mut self, event: &Value) {
        let Some(named) = event.pointer("/message/stopReason").and_then(Value::as_str) else {
            return;
        };
        if let Some(reason) = stop_reason_of(named) {
            self.stop_reason = Some(reason);
        }
    }

    /// `input + cacheRead + cacheWrite` — три счётчика `validity_gate.py:59`.
    fn take_usage(&mut self, event: &Value) {
        if let Some(usage) = event.get("usage") {
            self.total_input_tokens += usage_total(usage);
        }
    }
}

/// Три счётчика `validity_gate.py:59`. Счётчика, которого нет, — ноль: usage приходит
/// частичным объектом, и недостающий ключ не должен ронять разбор.
fn usage_total(usage: &Value) -> u64 {
    ["input", "cacheRead", "cacheWrite"]
        .iter()
        .filter_map(|key| usage.get(*key).and_then(Value::as_u64))
        .sum()
}

/// Имя причины от Pi. Что не названо — того и нет: `Timeout` раннер узнаёт из своих
/// часов, а не из потока, и подставлять его сюда нельзя.
fn stop_reason_of(named: &str) -> Option<StopReason> {
    match named {
        "stop" => Some(StopReason::Stop),
        "error" => Some(StopReason::Error),
        "aborted" => Some(StopReason::Aborted),
        _ => None,
    }
}

pub trait EventSource {
    fn next_line(&mut self) -> Option<String>;
}

/// Вердикт по накопленному прогону. Пороги — из `scripts/worker/dispatch.conf:73–75`
/// (`GATE_REQUIRE_STOP_OK`, `GATE_REQUIRE_TOOL_STEPS`, `GATE_REQUIRE_TURNS`); умолчания
/// 1/1/1 значат: остановка чистая, шаг есть, ходов больше одного.
///
/// Порядок проверок — как в `validity_gate.py`: сначала сам прогон, потом его
/// содержательность. Каждая причина — своё состояние; свёрнутая в «невалидно» ошибка
/// неотличима от невалидности по любой другой (закон 6).
pub fn verdict_for(stop: StopReason, tool_steps: u32, turns: u32) -> Verdict {
    if gate("GATE_REQUIRE_STOP_OK") > 0 && stop != StopReason::Stop {
        return Verdict::InvalidLoop;
    }
    let min_steps = gate("GATE_REQUIRE_TOOL_STEPS");
    if min_steps > 0 && tool_steps < min_steps {
        return Verdict::InvalidNoToolSteps;
    }
    // Порог = «ходов должно быть БОЛЬШЕ, чем это», а не «не меньше»: при 1
    // отбраковывается ход=1, ровно как `turns < 2` в validity_gate.py.
    let min_turns = gate("GATE_REQUIRE_TURNS");
    if min_turns > 0 && turns <= min_turns {
        return Verdict::InvalidTooFewTurns;
    }
    Verdict::Valid
}

/// Порог гейта из окружения — под теми же именами, что в `dispatch.conf`, чтобы порог
/// не разошёлся с диспетчером. Нечитаемое значение — встроенный порог, а не «проверка
/// выключена»: сломанная настройка обязана ужесточать гейт, а не выдавать `Valid`.
fn gate(name: &str) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|raw| raw.trim().parse().ok())
        .unwrap_or(1)
}
