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
use stellwerk_core::contract::{Event, EventRisk, EventSource, RiskClass, TaskSpec};

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
    journal.push("run.start", EventRisk::Classified(RiskClass::R1), &[]);
    for step in steps {
        let tool = step.tool.as_str();
        journal.push(
            "run.step",
            EventRisk::Classified(step.kind.risk()),
            &[("tool", tool)],
        );
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
        EventRisk::Classified(peak),
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
            EventRisk::Classified(RiskClass::R1),
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
    fn push(&mut self, event_type: &str, risk: EventRisk, extra: &[(&str, &str)]) {
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
///
/// `pub(crate)`, а не `fn`: чек вызова (`runner::record`, M4-06) тоже несёт метку
/// времени, и второй такой форматировщик в том же крете разошёлся бы с этим по
/// всем краям — включая трактовку часов ДО 1970.
pub(crate) fn now_rfc3339() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    rfc3339_from_secs(secs)
}

/// Метка времени из СЕКУНД, а не из часов машины: impure остаётся один, на
/// `SystemTime::now()`. Разбор суток проверяется на полночи и на границе суток — там
/// `tod / 3600`, `(tod % 3600) / 60` и `tod % 60` расходятся между собой, а в середине
/// дня все три дают одно и то же.
fn rfc3339_from_secs(secs: i64) -> String {
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

/// Тесты дат и суток живут в файле: `civil_from_days` приватна, а расширять её до `pub`
/// ради теста — значит менять API крейта. `SystemTime` не подменяется: impure остался
/// только на `now_rfc3339`, а обе проверяемые функции чисты, поэтому ездят на явных
/// входах, а не на часе машины.
#[cfg(test)]
mod tests {
    use super::{civil_from_days, rfc3339_from_secs, stop_name, verdict_name, Verdict};
    use crate::runner::StopReason;

    /// Границы, на которых ломается арифметика Хиннанта: эпоха и её соседи, дни ДО
    /// эпохи, вековые переносы 1900 и 2100 (оба не високосные), границы 400-летней эры
    /// (каждая ровно 146 097 дней, начало эры — 1 марта), месячные переходы.
    /// Пары «день ↔ дата» взяты из календаря, а НЕ выведены из `civil_from_days`:
    /// взаимно ошибочная пара прошла бы проверку молча.
    const DAYS: &[(i64, i64, u32, u32)] = &[
        // эпоха и её соседи
        (0, 1970, 1, 1),
        (1, 1970, 1, 2),
        (-1, 1969, 12, 31),
        // дни ДО эпохи: 31-дневный месяц, начало года, високосный 1968
        (-31, 1969, 12, 1),
        (-365, 1969, 1, 1),
        (-731, 1968, 1, 1),
        // век НЕ високосный: за 28 февраля сразу идёт 1 марта
        (-25_509, 1900, 2, 28),
        (-25_508, 1900, 3, 1),
        (47_540, 2100, 2, 28),
        (47_541, 2100, 3, 1),
        // границы 400-летней эры: 29 февраля есть, эра — ровно 146 097 дней
        (-135_081, 1600, 2, 29),
        (-135_080, 1600, 3, 1),
        (11_016, 2000, 2, 29),
        (11_017, 2000, 3, 1),
        (157_113, 2400, 2, 29),
        (157_114, 2400, 3, 1),
        // месячные переходы: 28/29/30/31 день и конец года
        (30, 1970, 1, 31),
        (58, 1970, 2, 28),
        (59, 1970, 3, 1),
        (119, 1970, 4, 30),
        (729, 1971, 12, 31),
        (730, 1972, 1, 1),
        (789, 1972, 2, 29),
    ];

    /// Моменты суток: полночь (`tod = 0`), ровно час, полдень, конец суток, ровно сутки,
    /// две метки ДО эпохи и одна не круглая. Именно эти значения различают `tod / 3600`,
    /// `(tod % 3600) / 60` и `tod % 60` — в середине дня все три дают одно и то же, и
    /// тест на «середину дня» не убил бы ни одного из трёх. Ожидания — литералы.
    const STAMPS: &[(i64, &str)] = &[
        (0, "1970-01-01T00:00:00Z"),
        (3_600, "1970-01-01T01:00:00Z"),
        (43_200, "1970-01-01T12:00:00Z"),
        (86_399, "1970-01-01T23:59:59Z"),
        (86_400, "1970-01-02T00:00:00Z"),
        (-1, "1969-12-31T23:59:59Z"),
        (-86_400, "1969-12-31T00:00:00Z"),
        (1_234_567_890, "2009-02-13T23:31:30Z"),
    ];

    /// Обратная к `civil_from_days` пересборка дня ДРУГИМ алгоритмом: порядковый номер
    /// года, таблица месяцев и правило високосности. Общее с проверяемой функцией
    /// только имя на входе. `div_euclid`, а не `/`: у отрицательных годов целочисленное
    /// деление в Rust обрезает к нулю, а ординалу нужен пол.
    fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
        const CUM: [i64; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
        let leap_seen = |y: i64| y.div_euclid(4) - y.div_euclid(100) + y.div_euclid(400);
        let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
        let ordinal = 365 * (year - 1970) + leap_seen(year - 1) - leap_seen(1969);
        ordinal + CUM[month as usize - 1] + i64::from(day) - 1 + i64::from(leap && month > 2)
    }

    /// Приёмка 1 — известные пары: названные границы дают названную дату. Ловит неверную
    /// константу (`719_468`, `146_097`, `146_096`, `36_524`, `1460`, `153`, `5`, `400`).
    /// Оракул заякорен на эту же таблицу, поэтому «неверная дата, неверный день» в неё
    /// не пройдёт.
    #[test]
    fn every_named_boundary_is_a_known_day_and_date_pair() {
        for &(days, year, month, day) in DAYS {
            assert_eq!(
                civil_from_days(days),
                (year, month, day),
                "день {days} дал не ту дату"
            );
            assert_eq!(
                days_from_civil(year, month, day),
                days,
                "дата {year}-{month}-{day} дала не тот день"
            );
        }
    }

    /// Приёмка 2 — round-trip: пять эр в обе стороны от эпохи, ДЕНЬ ЗА ДНЁМ, ±730_485 =
    /// ±5 · 146_097. Ловит переставленные операнды, которые на двадцати названных днях
    /// могли бы и совпасть. Оракул построен другим алгоритмом и повторяет все пары
    /// packet-21 из `session.rs` (35 штук, ноль расхождений) — общая с проверяемой
    /// функцией только формула входа.
    #[test]
    fn five_eras_in_both_directions_come_back_to_the_same_day() {
        for days in -730_485..=730_485 {
            let (year, month, day) = civil_from_days(days);
            assert_eq!(
                days_from_civil(year, month, day),
                days,
                "день {days} разобрался в {year}-{month}-{day} и обратно не собрался"
            );
        }
    }

    /// Приёмка 3 — разбор суток: `tod = 0`, ровно час, ровно сутки и максимум дня плюс
    /// день до эпохи, где знак минус обязан дать полночь ТОГО ЖЕ дня. Метки —
    /// литералы, а не та же сборка, что и печать.
    #[test]
    fn the_clock_divisions_disagree_exactly_at_the_named_boundaries() {
        for &(secs, stamp) in STAMPS {
            assert_eq!(rfc3339_from_secs(secs), stamp, "секунды {secs}");
        }
    }

    /// Имя отказа в payload — единственное, чем годный прогон отличается от отказного
    /// (закон 6), поэтому проверены ВСЕ имена, включая «valid»: пустая строка вместо
    /// имени не отличима от «имени нет».
    #[test]
    fn every_verdict_and_stop_reason_carries_its_own_name() {
        assert_eq!(
            [
                verdict_name(Verdict::Valid),
                verdict_name(Verdict::InvalidLoop),
                verdict_name(Verdict::InvalidNoToolSteps),
                verdict_name(Verdict::InvalidTooFewTurns)
            ],
            ["valid", "loop", "no_tool_steps", "too_few_turns"]
        );
        assert_eq!(
            [
                stop_name(StopReason::Stop),
                stop_name(StopReason::Error),
                stop_name(StopReason::Timeout),
                stop_name(StopReason::Aborted),
                stop_name(StopReason::Unreported)
            ],
            ["stop", "error", "timeout", "aborted", "unreported"]
        );
    }
}
