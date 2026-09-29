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
//! RED M2/packet-11: тела — `unimplemented!()` (реализация — M2/packet-12).

use crate::runner::RunOutcome;
use stellwerk_core::contract::{Event, RiskClass, TaskSpec};

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
        unimplemented!()
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
    let _ = (spec, outcome, steps);
    unimplemented!()
}
