//! Чек вызова: письменная запись каждого вызова инструмента.
//!
//! Gateway инструменты не выполняет — он решает, что можно, и пишет, что было
//! решено. Запись ведётся для КАЖДОГО вызова, включая запрещённые: запрет — тоже
//! факт, обязанный попасть в поток, иначе «тихие» попытки не видны в
//! расследовании. Вызов без чека не состоялся: без единственного следа в потоке
//! вызов неотличим от небывшего.
//!
//! В чек идут только sha256 от тел входа и вывода, никогда сами тела: секрет из
//! аргументов не уезжает в поток (REVIEW §3.2). Хеш пустого тела — канонический
//! дайджест, а не `""`: пустая строка означала бы «не посчитали».
//!
//! RED-заглушка M3/packet-09: поверхность объявлена, поведение — packet-10.

use std::collections::BTreeMap;

use thiserror::Error;

use stellwerk_core::contract::{Event, EventSource, RiskClass};

/// Решение по вызову. Реэкспорт из `gate`, а не второе объявление: вердикт в чеке
/// и вердикт `decide` обязаны быть одним типом, иначе чек мог бы оказаться про
/// вызов, которого не было. `Suspended` отделён от `Deny` (В4): «никто не
/// ответил» и «запрещено политикой» различимы и в чеке, и в потоке.
pub use crate::gate::Verdict;

/// Чек вызова: всё, что известно о вызове после решения.
#[derive(Debug, Clone)]
pub struct Receipt {
    /// Идентификатор вызова: по нему чек отыскивается в потоке.
    pub call_id: String,
    /// Имя инструмента.
    pub tool: String,
    /// MCP-модуль, из которого инструмент; он же `actor` события.
    pub module: String,
    /// Решение по вызову. `Deny` — тоже чек, а не отсутствие чека.
    pub verdict: Verdict,
    /// Класс риска вызова: в поток уходит тот же, что и в решении.
    pub risk: RiskClass,
    /// SHA-256 тела входа, hex в нижнем регистре. Само тело в чек не пишется.
    pub input_hash: String,
    /// SHA-256 полного тела вывода ДО shaping: хеш по усечённому тексту не
    /// сверить с артефактом (REVIEW §3.3).
    pub output_hash: String,
    /// Вывод усечён, полное тело ушло в артефакт.
    pub truncated: bool,
    /// Длительность вызова в миллисекундах. `u64`, а не `f64`: время не дробное.
    pub duration_ms: u64,
    /// Время вызова в RFC 3339 (UTC) — формат, которую ждёт `Event`.
    pub ts: String,
}

/// Что может помешать записи чека.
#[derive(Debug, Error)]
pub enum ReceiptError {
    /// Поведение не реализовано; до M3/packet-10 чека в потоке не существует.
    #[error("not implemented: чек вызова")]
    NotImplemented,
    /// Тело не отхешировалось. Хеш при этом не заполняется: пустым он быть не
    /// может, а «не посчитали» — отдельное состояние, а не значение.
    #[error("hash: {0}")]
    Hash(String),
}

impl Receipt {
    /// Превратить чек в событие потока: `gateway.call` для задачи `task_id`.
    ///
    /// Единственный путь вызова в поток. `task_id` обязателен именно потому,
    /// что `append` кладёт `payload["task_id"]` в отдельную колонку, а
    /// `list_by_task` выбирает по ней (stellwerk-store, `sqlite.rs`): чек без
    /// `task_id` не находился бы, и вызов без следа неотличим от небывшего.
    /// `workspace` заполняется: федеративным (`None`) чек вызова не бывает.
    /// Он приходит аргументом, а не константой модуля: подстановка константы была
    /// единственным выходом, пока значение было невыразимо, но чек с чужой
    /// константой выглядит полным, указывает не туда, где был вызов, и при одном
    /// рабочем месте неотличим от настоящего значения. Пустой строкой это не
    /// выражается: она означала бы «не посчитали», а чек без места не считается.
    /// RED: поведение — M3/packet-10.
    pub fn to_event(&self, task_id: &str, workspace: &str) -> Event {
        let mut payload = BTreeMap::new();
        payload.insert("task_id".to_owned(), task_id.to_owned());
        payload.insert("call_id".to_owned(), self.call_id.clone());
        payload.insert("tool".to_owned(), self.tool.clone());
        // `Debug` выбран не по красоте, а потому что он БЕЗ ПОТЕРЬ: `Deny`
        // уносит причину, а причина обязана дойти до потока. Схема без потерь,
        // иначе расследование не отличит «запрещено политикой» от «не спросили».
        payload.insert("verdict".to_owned(), format!("{:?}", self.verdict));
        payload.insert("truncated".to_owned(), self.truncated.to_string());
        payload.insert("input_hash".to_owned(), self.input_hash.clone());
        payload.insert("output_hash".to_owned(), self.output_hash.clone());
        payload.insert("duration_ms".to_owned(), self.duration_ms.to_string());

        Event {
            id: self.call_id.clone(),
            source: EventSource::System,
            event_type: "gateway.call".to_owned(),
            actor: self.module.clone(),
            workspace: Some(workspace.to_owned()),
            risk: self.risk,
            payload,
            ts: self.ts.clone(),
        }
    }
}
