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

use std::future::Future;
use std::pin::Pin;

use crate::sandbox::Sandbox;
use serde_json::Value;
use stellwerk_core::contract::{DataClass, ModelBinding, PolicyLevel, TaskSpec};
use stellwerk_gateway::gate::{decide, CallRequest};
use stellwerk_gateway::permission::ToolRisk;
use stellwerk_store::EventStore;
use thiserror::Error;

/// Решение по вызову — РЕЭКСПОРТ вердикта гейта, а не второе объявление: перевод
/// между двумя типами рано или поздно принёс бы `unwrap_or(R0)` (закон 6).
pub use stellwerk_gateway::gate::Verdict as CallVerdict;

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
    /// Входные токены вместе с кэшем — три счётчика `validity_gate.py:81-83`.
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
    /// Провайдер не сообщил причину остановки. Наблюдаемый факт, а не наше непонимание:
    /// `Stop` значит «поток сам объявил успех», и молчание при коде 0 — не то же самое
    /// (закон 6).
    Unreported,
}

/// Факты процесса, из которых выводится причина остановки. Отдельная структура, а не
/// три позиционных параметра: вызов будет стоять в теле запуска, где спешка обычна.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StopFacts {
    /// Сработал наш таймаут.
    pub timed_out: bool,
    pub exit_code: i32,
    /// Что провайдер назвал в потоке, если назвал.
    pub named: Option<StopReason>,
}

/// Причина остановки из фактов процесса. Порядок обязателен и идёт от нашего наблюдения
/// к чужому: таймаут перебивает код возврата, код возврата — слово провайдера. Убитый
/// нами прогон не бывает успешным, даже если процесс успел вернуть 0.
pub fn stop_reason_from(facts: StopFacts) -> StopReason {
    if facts.timed_out {
        return StopReason::Timeout;
    }
    if facts.exit_code != 0 {
        return StopReason::Error;
    }
    // Ни таймаута, ни сбоя, ни имени: сказать «модель дошла» здесь нечем, а `Stop`
    // значит ровно это. Подставить его — выдуманный default, запрещённый законом 6.
    facts.named.unwrap_or(StopReason::Unreported)
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

    /// Счётчики из одного события. Наборы доноров — измеренные на canary, а не выбранные:
    /// гейт считает шаг по `tool_execution_start` (40) и ход по `message_end` (77), тогда как
    /// концов хода всего 35. Считать оба счётчика по `turn_end` — считать одно и то же дважды.
    fn count(&mut self, event: &Value) {
        match event.get("type").and_then(Value::as_str) {
            Some("tool_execution_start") => self.tool_steps += 1,
            Some("message_end") => self.turns += 1,
            // `turn_end` шага не даёт — это единственный оставшийся донор: причина остановки
            // и usage. Ось причин уже совпадала с гейтом и остаётся нетронутой.
            Some("turn_end") => {
                self.take_stop(event);
                self.take_usage(event);
            }
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

    /// `message.usage` на `turn_end` — донор контекста в гейте (`validity_gate.py:81-83`).
    /// Читать `event.usage` из `message_update` — другой набор доноров: на canary это
    /// 1 105 242 против 655 622, и разница не в арифметике, а в том, ЧТО сложено.
    fn take_usage(&mut self, event: &Value) {
        if let Some(usage) = event.pointer("/message/usage") {
            self.total_input_tokens += usage_total(usage);
        }
    }
}

/// Три счётчика `validity_gate.py:81-83`. Счётчика, которого нет, — ноль: usage приходит
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

/// Вызов инструмента: всё, о чём раннер обязан спросить разрешение. Умолчаний нет:
/// «не классифицирован» это `Unclassified`, а не «риск не задан, значит нулевой».
#[derive(Debug, Clone)]
pub struct Call {
    pub tool: String,
    pub module: String,
    /// Риск инструмента. `Unclassified` — состояние, а не нулевой риск: иначе
    /// неизвестный тул проходил бы как `R0`, и дыра была бы неотличима от чтения.
    pub risk: ToolRisk,
    pub data: DataClass,
    pub model: ModelBinding,
    /// Есть ли кому задать вопрос. `false` — неинтерактивный прогон, и отсутствие
    /// ответа обязано стать `Suspended`, а не тихим `Allow` (В4).
    pub interactive: bool,
}

/// Решение по вызову: единственный вопрос раннера к слою разрешений. Тело — сборка
/// запроса и один вызов `decide`, без разбора вердикта: вторая копия правил прошла бы
/// тесты сегодня и разошлась с гейтом при первой же правке политики — ровно тот класс,
/// из которого вырос F-10.
pub fn authorize(call: &Call, caller: PolicyLevel) -> CallVerdict {
    let request = CallRequest {
        tool: call.tool.clone(),
        module: call.module.clone(),
        risk: call.risk,
        caller,
        data: call.data,
        model: call.model.clone(),
        interactive: call.interactive,
    };
    decide(&request)
}

// --- M4/packet-05: вертикальный срез — решение обязано дойти до того, что произойдёт ---

/// Попытка вызова: что собираются запустить, где и с какими полномочиями.
///
/// Отдельная от `RunRequest` (M2-09) сущность: прогон делает МНОГО вызовов, и один
/// `RunRequest` на попытку не годится — `task_id` пришлось бы тащить в спецификацию
/// прогона, а чек писать нечем.
#[derive(Debug, Clone)]
pub struct CallAttempt {
    pub task_id: String,
    pub call: Call,
    pub caller: PolicyLevel,
    pub workspace_ref: String,
}

/// Итог одного вызова: вывод рабочего места. Тела вывода в чек не идут — только хеш
/// (пакет 06), и потому здесь нечего, кроме самого текста.
#[derive(Debug, Clone)]
pub struct ExecOutcome {
    pub output: String,
}

/// Сбой рабочего места. Newtype над чужим текстом, а НЕ перечень причин: опись того,
/// как чужой клиент падает, стала бы вторым источником истины о нём самом.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{0}")]
pub struct WorkerError(pub String);

/// Отказ прохода. Отдельным состоянием, а не `Allow`: упавший вызов, отчитавшийся
/// успехом, неотличим от состоявшегося. Причина обязана называться — иначе сбой
/// нечем расследовать.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{0}")]
pub struct RunError(pub String);

/// Шов запуска. `spawned()` отделён от `EventStore` намеренно: «воркер не вызван»
/// иначе пришлось бы проверять по отсутствию события, а отсутствие события
/// неотличимо от «событие записать забыли» — и такой тест прошёл бы при молчащем
/// раннере, то есть не проверял бы ничего.
pub trait Worker {
    /// Вызвать инструмент. Future вместо `async fn`: в публичном трейте `async fn` не
    /// даёт задать `Send`, а ADR-0001 разбирает ровно эту неразвязку; `ToolBackend` в
    /// gateway объявлен так же.
    fn exec(
        &self,
        workspace_ref: &str,
        call: &Call,
    ) -> Pin<Box<dyn Future<Output = Result<ExecOutcome, WorkerError>> + Send + '_>>;

    /// Сколько раз воркер ФАКТИЧЕСКИ запускали, включая упавшие запуски.
    fn spawned(&self) -> usize;
}

/// Один проход: решение → запуск или отказ → чек.
///
/// Чек пишется на КАЖДЫЙ вызов, включая запрещённый: вызов без следа неотличим от
/// небывшего, и «тихие» попытки исчезают из расследования.
///
/// `async`, а не `pub fn`: `Worker::exec` и `EventStore::append` асинхронны, и
/// синхронная подпись не дождалась бы ни того, ни другого.
///
/// `&impl EventStore`, а НЕ `&dyn EventStore`, как в пакете: трейт возвращает
/// `impl Future`, а такой метод не помещается в vtable — `&dyn EventStore` не
/// компилируется вовсе (ADR-0001 выбрал `impl Future` ради явного `Send`).
/// RED M4/packet-05: тело — `unimplemented!()` (реализация — M4/packet-06).
pub async fn run_once(
    attempt: &CallAttempt,
    worker: &dyn Worker,
    store: &impl EventStore,
) -> Result<CallVerdict, RunError> {
    let _ = (worker, store);
    unimplemented!(
        "M4/packet-06: тело прохода для `{}` не написано",
        attempt.task_id
    )
}
