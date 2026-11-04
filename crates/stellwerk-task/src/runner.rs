//! # Запуск воркера и разбор потока Pi
//!
//! Раннер — граница, где чужой процесс становится событиями федерации: Pi стартует
//! ВНУТРИ песочницы (`command` отдаёт непрочитанную команду, чтобы отказ программы
//! не слился с отказом песочницы), а его stdout читается построчно.
//!
//! Поток — НЕдоверенный ввод, и строка может оказаться не-JSON. Молча пропустить её
//! нельзя: прогон, потерявший шаг, неотличим от прогона без шага (закон 6).
//!
//! RED M2/packet-09: тела — `unimplemented!()` (реализация — M2/packet-10).

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::events::now_rfc3339;
use crate::evidence::{evidence_status, EvidenceEntry, EvidenceLedger, EvidenceStatus};
use crate::m5::sandbox_bwrap::Sandbox;
use serde_json::Value;
use sha2::{Digest, Sha256};
use stellwerk_core::contract::{
    DataClass, Event, EventRisk, ModelBinding, PolicyLevel, TaskSpec, TaskStatus,
};
use stellwerk_gateway::gate::{decide, CallRequest};
use stellwerk_gateway::permission::{PermissionError, ToolRisk};
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

/// Вызов инструмента: всё, о чём раннер обязан спросить разрешение.
#[derive(Debug, Clone)]
pub struct Call {
    pub tool: String,
    pub module: String,
    /// Заявленный вызывающим риск. НЕ влияет на разрешение: эффективный риск берёт
    /// из доверенного реестра `authorize`, а самодекларация оставлена, чтобы тесты
    /// могли доказать её незначимость (закон 6). Поле читается только здесь, в
    /// тестах и в старых вызывающих, и ни один путь решения его не использует.
    pub risk: ToolRisk,
    pub data: DataClass,
    pub model: ModelBinding,
    /// Есть ли кому задать вопрос. `false` — неинтерактивный прогон, и отсутствие
    /// ответа обязано стать `Suspended`, а не тихим `Allow` (В4).
    pub interactive: bool,
}

/// Реестр рисков — единственный источник эффективного риска на производственном пути.
/// Ошибка реестра или отсутствие записи — не повод подставлять `R0`.
pub trait RiskRegistry: Send + Sync {
    fn risk_of(&self, tool: &str, module: &str) -> Result<ToolRisk, PermissionError>;
}

/// Решение гейта с применённым риском из доверенного реестра.
#[derive(Debug, Clone)]
pub struct GateDecision {
    pub risk: ToolRisk,
    pub verdict: CallVerdict,
}

/// Решение по вызову: единственный вопрос раннера к слою разрешений. Тело — сборка
/// запроса из ДОВЕРЕННОГО реестра и контекста сессии и один вызов `decide`.
///
/// `call.risk` не читается: раньше именно копирование самодекларации в запрос
/// делало вызывающего источником эффективного риска, и решение соглашалось с
/// объявлением, а не с реестром. Ошибка реестра — отказ ДО исполнения; отсутствие
/// записи (`Unclassified`) решает `decide` по общей политике, а не `R0`-умолчание.
pub fn authorize(call: &Call, caller: PolicyLevel, registry: &dyn RiskRegistry) -> GateDecision {
    let applied = match registry.risk_of(&call.tool, &call.module) {
        Ok(risk) => risk,
        Err(err) => {
            return GateDecision {
                risk: ToolRisk::Unclassified,
                verdict: CallVerdict::Deny {
                    reason: format!(
                        "registry_refused: инструмент `{}`/`{}` не разрешён доверенным реестром: {err}",
                        call.tool, call.module
                    ),
                },
            };
        }
    };
    let request = CallRequest {
        tool: call.tool.clone(),
        module: call.module.clone(),
        risk: applied,
        caller,
        data: call.data,
        model: call.model.clone(),
        interactive: call.interactive,
    };
    GateDecision {
        risk: applied,
        verdict: decide(&request),
    }
}

/// Куда ведёт приостановка. Вопрос человеку есть только у `Ask`, поэтому только он
/// разрешает `AwaitingHumanValidation`. `Suspended` — «спросить некого» — остаётся
/// отдельным состоянием паузы и в ожидание решения не превращается: тишина не
/// становится ни разрешением, ни вопросом (закон 6).
///
/// `None` — вердикт прогон не приостанавливает. Не `unwrap_or(TaskStatus::Suspended)`:
/// неприменимый вердикт не должен молча выглядеть как сознательная пауза.
pub fn suspension_status(verdict: &CallVerdict) -> Option<TaskStatus> {
    match verdict {
        CallVerdict::Ask => Some(TaskStatus::AwaitingHumanValidation),
        CallVerdict::Suspended => Some(TaskStatus::Suspended),
        CallVerdict::Allow | CallVerdict::Deny { .. } => None,
    }
}

/// Возобновление после человеческой валидации: `HumanValidated` возвращается только
/// для записи, принадлежащей ЭТОЙ задаче. `entries(task_id)` не считается фильтром —
/// реализация вправе отдать больше (протекающий ledger), поэтому чужие записи
/// отбрасываются здесь. Иначе валидация другой задачи возобновила бы прогон,
/// которого человек не видел.
pub fn resume_after_human_validation(ledger: &dyn EvidenceLedger, task_id: &str) -> EvidenceStatus {
    let chain: Vec<EvidenceEntry> = ledger
        .entries(task_id)
        .into_iter()
        .filter(|entry| entry.task_id == task_id)
        .collect();
    evidence_status(&chain)
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

    /// Доверенный реестр рисков, используемый для принятия решения.
    fn registry(&self) -> &dyn RiskRegistry;
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
/// GREEN M4/packet-06: решение → чек на ЛЮБОЙ вызов → запуск только при `Allow`.
pub async fn run_once(
    attempt: &CallAttempt,
    worker: &dyn Worker,
    store: &impl EventStore,
) -> Result<CallVerdict, RunError> {
    let decision = authorize(&attempt.call, attempt.caller, worker.registry());
    match decision.verdict {
        CallVerdict::Allow => match worker.exec(&attempt.workspace_ref, &attempt.call).await {
            Ok(outcome) => {
                let digest = format!("{:x}", Sha256::digest(outcome.output.as_bytes()));
                record(
                    store,
                    attempt,
                    decision.risk,
                    CallStatus::Succeeded,
                    None,
                    Some(&digest),
                )
                .await?;
                Ok(CallVerdict::Allow)
            }
            Err(failure) => {
                let cause = failure.0;
                match record(
                    store,
                    attempt,
                    decision.risk,
                    CallStatus::Failed,
                    Some(&cause),
                    None,
                )
                .await
                {
                    Ok(()) => Err(RunError(cause)),
                    Err(unwritten) => Err(RunError(format!(
                        "чек сбоя не записан: {unwritten}; причина сбоя: {cause}"
                    ))),
                }
            }
        },
        CallVerdict::Deny { reason } => {
            record(
                store,
                attempt,
                decision.risk,
                CallStatus::Denied,
                Some(&reason),
                None,
            )
            .await?;
            // Отказ уходит в карантин отдельной записью: чек `run.call` говорит
            // «вызов не состоялся», а карантин — «его артефакт не станет
            // кандидатом» со своей причиной. Слить их значило бы потерять одно
            // из двух состояний.
            quarantine(store, attempt, decision.risk, &reason).await?;
            Ok(CallVerdict::Deny { reason })
        }
        CallVerdict::Suspended => {
            record(
                store,
                attempt,
                decision.risk,
                CallStatus::Suspended,
                Some(NOBODY_TO_ASK),
                None,
            )
            .await?;
            Ok(CallVerdict::Suspended)
        }
        CallVerdict::Ask => {
            record(
                store,
                attempt,
                decision.risk,
                CallStatus::Asked,
                Some(ANSWER_PENDING),
                None,
            )
            .await?;
            Ok(CallVerdict::Ask)
        }
    }
}

/// Итог прохода по запрошенным вызовам. «Дошёл до конца» и «остановлен» — разные
/// состояния (закон 6), поэтому это вариант, а не длина вектора.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatedPass {
    /// Пачка пройдена целиком, каждый вызов разрешён и исполнен.
    Completed(Vec<CallVerdict>),
    /// Проход остановлен первым не-`Allow`: вектор — обработанный префикс,
    /// последний элемент — остановивший вердикт, хвост пачки не запрашивался.
    Blocked(Vec<CallVerdict>),
}

/// Петля по запрошенным вызовам: решения выносятся по одному через `run_once`
/// (решение — до эффекта, эффект даёт только `Allow`).
///
/// Первый не-`Allow` (`Ask`/`Suspended`/`Deny`, в т.ч. при оборванном канале
/// реестра) записывается в чек и завершает проход — хвост пачки НЕ запрашивается:
/// он не исполнен, не одобрен и не появляется в потоке. Продолжение после ответа на
/// `Ask` — новый ход модели, а не повтор хвоста.
///
/// `Err` — сбой эффекта или записи чека, а не отказ гейта: отказ — отдельное
/// состояние-вердикт.
pub async fn run_gated(
    attempts: &[CallAttempt],
    worker: &dyn Worker,
    store: &impl EventStore,
) -> Result<GatedPass, RunError> {
    let mut verdicts = Vec::new();
    for attempt in attempts {
        let verdict = run_once(attempt, worker, store).await?;
        let allowed = matches!(verdict, CallVerdict::Allow);
        verdicts.push(verdict);
        if !allowed {
            return Ok(GatedPass::Blocked(verdicts));
        }
    }
    Ok(GatedPass::Completed(verdicts))
}

/// Состояние чека вызова. Пять разных фактов, а не «сработало / не сработало»:
/// «запрещено политикой», «никто не ответил», «вопрос задан», «состоялся» и «упал»
/// обязаны различаться в потоке, иначе расследование не отличит отказ от молчания
/// (В4, закон 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CallStatus {
    Denied,
    Suspended,
    /// Вопрос задан, ответа нет: `Ask` дошёл до слоя, который ответить не в состоянии.
    Asked,
    Succeeded,
    Failed,
}

impl CallStatus {
    /// Имя состояния в payload чека. Своя строка на каждое: слитое «invalid» не
    /// разбирается, а чек без разбора — просто ещё одна запись в потоке.
    fn name(self) -> &'static str {
        match self {
            CallStatus::Denied => "Denied",
            CallStatus::Suspended => "Suspended",
            CallStatus::Asked => "Asked",
            CallStatus::Succeeded => "Succeeded",
            CallStatus::Failed => "Failed",
        }
    }
}

/// Причина `Suspended` называется явно: пустая причина читалась бы как «причина не
/// нашлась», а это другое состояние (закон 6).
const NOBODY_TO_ASK: &str = "спросить некому: проход неинтерактивен, канала ответа нет";

/// Причина `Ask`: вопрос задан, но разрешить его в этом проходе нечем. Интерактивный
/// путь заводит M4-07, и до него `Ask` остаётся фактом, а не тихим `Allow`.
const ANSWER_PENDING: &str = "вопрос задан, ответ не получен: канал ответа появится в M4-07";

/// Порядковый номер чека. Счётчик, а не хеш содержимого: два одинаковых повторных
/// вызова в одной задаче дали бы одинаковый хеш, а `append` отвергает повторный `id`
/// (уникальный индекс в `events`) — и второй честный вызов стал бы отказом
/// хранилища. После перезапуска процесса счётчик идёт с нуля: коллизия пришла бы
/// ошибкой, а не тихой перезаписью чужого следа.
static RECEIPT_SEQ: AtomicU64 = AtomicU64::new(0);

/// Чек одного вызова: единственное место, где раннер пишет событие потока.
///
/// Ключи payload названы здесь, а не в тесте: тест, выдумывающий схему, заставил бы
/// следующий пакет повторить её ещё раз, и две схемы разошлись бы молча.
///
/// `reason` и `output_sha256` — `Option`: у состояния, которому нечего туда положить,
/// полей НЕТ. Пустая строка означала бы «не посчитали», и честный `Succeeded` стал бы
/// неотличим от недосказанного (закон 6).
async fn record(
    store: &impl EventStore,
    attempt: &CallAttempt,
    applied_risk: ToolRisk,
    status: CallStatus,
    reason: Option<&str>,
    output_sha256: Option<&str>,
) -> Result<(), RunError> {
    let mut payload: BTreeMap<String, String> = BTreeMap::new();
    payload.insert("task_id".to_owned(), attempt.task_id.clone());
    payload.insert("tool".to_owned(), attempt.call.tool.clone());
    payload.insert("module".to_owned(), attempt.call.module.clone());
    payload.insert("caller".to_owned(), format!("{:?}", attempt.caller));
    payload.insert("status".to_owned(), status.name().to_owned());
    payload.insert("interactive".to_owned(), "false".to_owned());
    if let ToolRisk::Classified(class) = applied_risk {
        payload.insert("risk".to_owned(), format!("{class:?}"));
    }
    if let Some(reason) = reason {
        payload.insert("reason".to_owned(), reason.to_owned());
    }
    if let Some(digest) = output_sha256 {
        payload.insert("output_sha256".to_owned(), digest.to_owned());
    }
    let event = Event {
        id: format!(
            "{}-call-{:08}",
            attempt.task_id,
            RECEIPT_SEQ.fetch_add(1, Ordering::Relaxed)
        ),
        source: stellwerk_core::contract::EventSource::System,
        event_type: "run.call".to_owned(),
        actor: match status {
            CallStatus::Succeeded | CallStatus::Failed => attempt.call.module.clone(),
            CallStatus::Denied | CallStatus::Suspended | CallStatus::Asked => "policy".to_owned(),
        },
        workspace: Some(attempt.workspace_ref.clone()),
        risk: column_risk(applied_risk),
        payload,
        ts: now_rfc3339(),
    };
    store
        .append(event)
        .await
        .map_err(|err| RunError(format!("чек не записан: {err}")))
}

/// Тип события карантина. Отдельный от `run.call`: отказанный прогон не должен
/// читаться как состоявшийся вызов, а его артефакт — как успешный кандидат.
const QUARANTINE_EVENT: &str = "run.quarantine";

/// Срок хранения карантинной записи в секундах: семь суток. Срок конечный и явный,
/// но истечение — только состояние, а не повод удалить доказательство отказа.
pub const QUARANTINE_RETENTION_SECS: u64 = 7 * 24 * 60 * 60;

/// Состояние карантинной записи на момент чтения. `Expired` запись остаётся видимой:
/// удаление по таймеру уничтожило бы доказательство отказа.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuarantineState {
    Active,
    Expired,
}

/// Запись карантина: отказанный прогон, причина отказа и явные факты срока хранения.
/// Append-only — запись не удаляется и не перезаписывается; читается отдельно от
/// обычных чеков.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuarantineRecord {
    pub task_id: String,
    pub tool: String,
    pub module: String,
    /// Причина отказа. Пустой причины не бывает: запись без неё — не карантин,
    /// а потерянный отказ (закон 6).
    pub reason: String,
    /// Когда запись создана, epoch-секунды.
    pub recorded_at_secs: u64,
    /// Когда истекает срок хранения, epoch-секунды.
    pub expires_at_secs: u64,
    /// Активна или истекла на момент чтения; запись присутствует в обоих случаях.
    pub state: QuarantineState,
}

/// Порядковый номер карантинной записи. Свой счётчик, чтобы id не зависел от
/// последовательности обычных чеков: `append` отвергает повторный `id`.
static QUARANTINE_SEQ: AtomicU64 = AtomicU64::new(0);

/// Текущий момент в epoch-секундах. Ошибка часов — явный отказ, а не нулевое время:
/// `unwrap_or(0)` сделал бы любую запись «истёкшей с 1970 года».
fn current_epoch_secs() -> Result<u64, RunError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .map_err(|err| RunError(format!("время до epoch: {err}")))
}

/// Отправить отказанный прогон в карантин. Причина обязательна: отказ без неё —
/// это не запись карантина, а его потеря.
async fn quarantine(
    store: &impl EventStore,
    attempt: &CallAttempt,
    applied_risk: ToolRisk,
    reason: &str,
) -> Result<(), RunError> {
    if reason.trim().is_empty() {
        return Err(RunError(format!(
            "карантин без причины: отказ по `{}`/`{}` не записан",
            attempt.call.tool, attempt.call.module
        )));
    }
    let recorded_at_secs = current_epoch_secs()?;
    let expires_at_secs = recorded_at_secs
        .checked_add(QUARANTINE_RETENTION_SECS)
        .ok_or_else(|| {
            RunError(format!(
                "срок хранения карантина переполнен: {recorded_at_secs}"
            ))
        })?;
    let mut payload: BTreeMap<String, String> = BTreeMap::new();
    payload.insert("task_id".to_owned(), attempt.task_id.clone());
    payload.insert("tool".to_owned(), attempt.call.tool.clone());
    payload.insert("module".to_owned(), attempt.call.module.clone());
    payload.insert("reason".to_owned(), reason.to_owned());
    payload.insert("recorded_at_secs".to_owned(), recorded_at_secs.to_string());
    payload.insert("expires_at_secs".to_owned(), expires_at_secs.to_string());
    let event = Event {
        id: format!(
            "{}-quarantine-{:08}",
            attempt.task_id,
            QUARANTINE_SEQ.fetch_add(1, Ordering::Relaxed)
        ),
        source: stellwerk_core::contract::EventSource::System,
        event_type: QUARANTINE_EVENT.to_owned(),
        actor: "policy".to_owned(),
        workspace: Some(attempt.workspace_ref.clone()),
        risk: column_risk(applied_risk),
        payload,
        ts: now_rfc3339(),
    };
    store
        .append(event)
        .await
        .map_err(|err| RunError(format!("карантин не записан: {err}")))
}

/// Прочитать карантин задачи на текущий момент. Тонкая обёртка над
/// `quarantine_entries_at`, чтобы вызывающий не подставлял часы руками.
pub async fn quarantine_entries(
    store: &impl EventStore,
    task_id: &str,
) -> Result<Vec<QuarantineRecord>, RunError> {
    let now_secs = current_epoch_secs()?;
    quarantine_entries_at(store, task_id, now_secs).await
}

/// Прочитать карантин задачи на ЯВНЫЙ момент `now_secs`. Истечение срока — производное
/// состояние чтения: запись навсегда остаётся в append-only следе, а `now_secs`
/// позволяет проверить границу без сна и без подмены системных часов.
///
/// Запись без причины, идентичности вызова или фактов срока — ошибка чтения, а не
/// пропуск и не умолчание: отброшенная запись неотличима от несуществующей.
pub async fn quarantine_entries_at(
    store: &impl EventStore,
    task_id: &str,
    now_secs: u64,
) -> Result<Vec<QuarantineRecord>, RunError> {
    let events = store
        .list_by_task(task_id)
        .await
        .map_err(|err| RunError(format!("карантин не прочитан: {err}")))?;
    events
        .into_iter()
        .filter(|event| event.event_type == QUARANTINE_EVENT)
        .map(|event| {
            let field = |name: &str| {
                event
                    .payload
                    .get(name)
                    .filter(|value| !value.trim().is_empty())
                    .cloned()
                    .ok_or_else(|| {
                        RunError(format!("карантинная запись `{}` без `{name}`", event.id))
                    })
            };
            let number = |name: &str| {
                let raw = field(name)?;
                raw.parse::<u64>().map_err(|_| {
                    RunError(format!(
                        "карантинная запись `{}` с нечисловым `{name}`: {raw:?}",
                        event.id
                    ))
                })
            };
            let recorded_at_secs = number("recorded_at_secs")?;
            let expires_at_secs = number("expires_at_secs")?;
            if expires_at_secs < recorded_at_secs {
                return Err(RunError(format!(
                    "карантинная запись `{}` истекает раньше, чем записана: {expires_at_secs} < {recorded_at_secs}",
                    event.id
                )));
            }
            let state = if now_secs > expires_at_secs {
                QuarantineState::Expired
            } else {
                QuarantineState::Active
            };
            Ok(QuarantineRecord {
                task_id: field("task_id")?,
                tool: field("tool")?,
                module: field("module")?,
                reason: field("reason")?,
                recorded_at_secs,
                expires_at_secs,
                state,
            })
        })
        .collect()
}

/// Риск события в колонке `Event.risk`. Раньше здесь стоял `R1` для
/// `ToolRisk::Unclassified` — то есть «никто не ответил» превращался в «риск высокий»,
/// и это значение ничем не отличалось от сознательно присвоенного R1. Теперь неизвестное
/// называется своим состоянием (`Unclassified`) и сериализуется в `"unclassified"`.
fn column_risk(risk: ToolRisk) -> EventRisk {
    match risk {
        ToolRisk::Classified(class) => EventRisk::Classified(class),
        ToolRisk::Unclassified => EventRisk::Unclassified,
    }
}
