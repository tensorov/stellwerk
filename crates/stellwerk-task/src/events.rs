//! # Запись событий прогона в хранилище
//!
//! Каждый шаг Pi обязан стать событием федерации: `RunOutcome` хранит только итоговые
//! счётчики, а журнал — единственное место, где видно, ЧТО прогон делал.
//!
//! Инварианты маппинга: `payload["task_id"]` обязателен — `append` выносит его в
//! колонку и `list_by_task` выбирает по ней; `id` уникален в прогоне и сортируется по
//! порядку записи (выборка упорядочена по `(ts, id)`, а `ts` не точнее секунды);
//! причина отказа вердикта лежит в payload (`loop`, `no_tool_steps`, `too_few_turns`),
//! иначе отказы слились бы в одно «невалидно» (закон 6). `workspace` —
//! `spec.workspace_ref`, `source` — `EventSource::System`.
//! GREEN M2/packet-12: вердикт приходит готовым — здесь он только НАЗЫВАЕТСЯ, решение
//! о нём принимает `runner::verdict_for`, и второе мнение о годности прогона разошлось
//! бы с первым молча.

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::runner::{RunOutcome, StopReason};
use stellwerk_core::contract::{Event, EventSource, RiskClass, TaskSpec};

/// `Verdict` объявлен в `runner` (M2-09) и здесь ТОЛЬКО реэкспортируется: вердикт о
/// прогоне один, второй `Verdict` разошёлся бы с первым по смыслу.
pub use crate::runner::Verdict;

/// Вид шага. Риск — свойство ВИДА шага, а не произвольное поле у события: иначе шаг
/// сети можно было бы записать как `R1` и потерять выход наружу.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    /// Правка файла в песочнице: R1, откатывается удалением песочницы.
    FileWrite,
    /// Сетевой вызов наружу: R2, наружу уходят только public-данные.
    Network,
}

impl StepKind {
    /// Единственное место, где вид шага превращается в риск: маппер копирует его, а
    /// не решает заново — иначе два решения разойдутся.
    pub fn risk(self) -> RiskClass {
        match self {
            StepKind::FileWrite => RiskClass::R1,
            StepKind::Network => RiskClass::R2,
        }
    }
}

/// Один шаг прогона. Номера шага здесь нет: порядок задаёт позиция в срезе, и счётчик
/// внутри шага разошёлся бы с ним.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub kind: StepKind,
    /// Имя вызванного инструмента: в payload шага, иначе шаг неопознаваем при разборе.
    pub tool: String,
}

/// События прогона в порядке записи: `run.start`, по `run.step` на шаг, затем `run.end`
/// и, при невалидном вердикте, `run.invalid` с причиной в payload.
pub fn events_for(spec: &TaskSpec, outcome: &RunOutcome, steps: &[Step]) -> Vec<Event> {
    let mut journal = Journal::new(spec);
    journal.push("run.start", RiskClass::R1, &[]);
    for step in steps {
        let tool = step.tool.as_str();
        journal.push("run.step", step.kind.risk(), &[("tool", tool)]);
    }
    // Конец прогона не тише самого рискованного шага: сеть наружу уже выходила, и тихий
    // `run.end` скрыл бы это от выборки по риску. Шагов не было — только старт, то есть
    // R1: «максимум по пустому набору» был бы выдуманным выходом наружу.
    let peak = steps
        .iter()
        .map(|s| s.kind.risk())
        .max()
        .unwrap_or(RiskClass::R1);
    journal.push(
        "run.end",
        peak,
        &[
            ("stop_reason", stop_name(outcome.stop_reason)),
            ("verdict", verdict_name(outcome.verdict)),
            ("tool_steps", &outcome.tool_steps.to_string()),
            ("turns", &outcome.turns.to_string()),
            (
                "total_input_tokens",
                &outcome.total_input_tokens.to_string(),
            ),
        ],
    );
    // Отказ пишется ТОЛЬКО при отказе: запись «невалидно» в валидном прогоне стёрла бы
    // разницу между «проверено и годно» и «не проверено» (закон 6).
    if let Some(reason) = verdict_reason(outcome.verdict) {
        journal.push(
            "run.invalid",
            RiskClass::R1,
            &[
                ("reason", reason),
                ("verdict", verdict_name(outcome.verdict)),
            ],
        );
    }
    journal.events
}

/// Писец прогона: спецификация, ОДНА метка времени на весь прогон, номер события и
/// накопленные события. Номер растёт сам при добавлении: ручной счётчик разошёлся бы с
/// порядком записи, а журнал врёт только в одну сторону — в меньшую.
struct Journal<'a> {
    spec: &'a TaskSpec,
    ts: String,
    written: usize,
    events: Vec<Event>,
}

impl<'a> Journal<'a> {
    /// Метка времени снимается один раз на прогон: события различаются номером, а не
    /// временем — иначе соседние события оказались бы в разных секундах, и порядок
    /// выбора зависел бы от скорости машины.
    fn new(spec: &'a TaskSpec) -> Self {
        Self {
            spec,
            ts: now_rfc3339(),
            written: 0,
            events: Vec::new(),
        }
    }

    /// Дописать событие прогона. `task_id` кладётся в payload ВСЕГДА: `append` выносит
    /// его в колонку, и без него событие не попадёт ни в одну выборку по задаче.
    ///
    /// Ширина номера в `id` фиксирована и с запасом: `id` сравнивается лексикографически,
    /// и переход с `9999` на `10000` переставил бы события конца прогона.
    fn push(&mut self, event_type: &str, risk: RiskClass, extra: &[(&str, &str)]) {
        let mut payload: BTreeMap<String, String> = extra
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        payload.insert("task_id".to_owned(), self.spec.id.clone());
        let id = format!("{}-{:08}", self.spec.id, self.written);
        self.written += 1;
        self.events.push(Event {
            id,
            source: EventSource::System,
            event_type: event_type.to_owned(),
            actor: self.spec.model_ref.clone(),
            workspace: Some(self.spec.workspace_ref.clone()),
            risk,
            payload,
            ts: self.ts.clone(),
        });
    }
}

/// Имя отказа вердикта. У `Valid` отказа НЕТ: `None`, а не `Some("valid")` — иначе годный
/// прогон и отказной были бы одной строкой в payload (закон 6).
fn verdict_reason(verdict: Verdict) -> Option<&'static str> {
    match verdict {
        Verdict::Valid => None,
        Verdict::InvalidLoop => Some("loop"),
        Verdict::InvalidNoToolSteps => Some("no_tool_steps"),
        Verdict::InvalidTooFewTurns => Some("too_few_turns"),
    }
}

/// Имя вердикта для `run.end`. Отказы берут те же имена, что и `verdict_reason`: две
/// таблицы имён разошлись бы, и разбор отказа стал бы ловушкой для читателя.
fn verdict_name(verdict: Verdict) -> &'static str {
    verdict_reason(verdict).unwrap_or("valid")
}

/// Имя причины остановки. `unreported` — наше слово о провайдерском молчании, а не имя
/// из потока Pi: назвать молчание `stop` значило бы выдумать решение провайдера, и прогон
/// без причины стал бы неотличим от прогона с причиной (закон 6).
fn stop_name(stop: StopReason) -> &'static str {
    match stop {
        StopReason::Stop => "stop",
        StopReason::Error => "error",
        StopReason::Timeout => "timeout",
        StopReason::Aborted => "aborted",
        StopReason::Unreported => "unreported",
    }
}

/// Метка времени в RFC 3339 UTC. Крейта времени в зависимостях нет, а контракт хранит
/// `ts` строкой: одну функцию форматирования дешевле написать, чем заводить зависимость
/// ради одного поля. Часы ДО 1970 — сломанные часы, и `1970-01-01T00:00:00Z` здесь значит
/// «время неизвестно», а не «момент запуска»: подставлять `now» в этот случай нельзя
/// (закон 6).
fn now_rfc3339() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let (year, month, day) = civil_from_days(secs.div_euclid(86_400));
    let tod = secs.rem_euclid(86_400);
    let (hour, min, sec) = (tod / 3600, (tod % 3600) / 60, tod % 60);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z")
}

/// Дата из дней от эпохи (алгоритм Хинанта): сдвиг на 719468 превращает начало года в
/// марте в начало счёта 1 марта 400 года, дальше год, месяц и день считаются целочисленно.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097) as u64; // [0, 146096]
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era as i64 + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153; // [0, 11], март = 0
    let day = (day_of_year - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if month <= 2 { year + 1 } else { year }, month, day)
}
