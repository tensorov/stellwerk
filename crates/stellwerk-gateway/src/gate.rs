//! Итоговое решение по вызову: риск инструмента, полномочия вызывающего и класс
//! данных сводятся в один `Verdict`.
//!
//! Пакет объявляет ВСЮ публичную поверхность; логику пишет packet-04 — иначе
//! тесты RED-пакета оказываются впереди своего типа.
//!
//! Правила принимаются ТОЛЬКО через предикаты контракта и `permission_for`.
//! `Permission` и `RiskClass` нужны телу `decide` (packet-04): в заглушке они
//! не используются, а неиспользуемый импорт сломал бы гейт `-D warnings`.

use crate::permission::ToolRisk;
use stellwerk_core::contract::{DataClass, ModelBinding, PolicyLevel};

/// Запрос на вызов инструмента: всё, о чём gateway знает до решения.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallRequest {
    /// Имя инструмента: в `Ask` человек читает его.
    pub tool: String,
    /// MCP-модуль, из которого инструмент.
    pub module: String,
    /// Риск инструмента. `Unclassified` — отдельное состояние, а не нулевой риск.
    pub risk: ToolRisk,
    /// Уровень полномочий вызывающего.
    pub caller: PolicyLevel,
    /// Класс чувствительности данных, уходящих по этому вызову.
    pub data: DataClass,
    /// Binding модели, которая их обработает.
    pub model: ModelBinding,
    /// Есть ли кому задать вопрос. `false` — неинтерактивный прогон.
    pub interactive: bool,
}

/// Решение по вызову.
///
/// `Suspended` отделён от `Deny` намеренно: «запрещено политикой» и «никто не
/// ответил» — разные ситуации (В4), и в статусе они должны различаться.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Разрешено.
    Allow,
    /// Спросить человека; без интерактива превращается в `Suspended`.
    Ask,
    /// Вопрос задан, ответа нет.
    Suspended,
    /// Отказ. Причина обязательна: без неё агент не поймёт, что делать.
    Deny { reason: String },
}

/// Итоговый вердикт по вызову. RED-заглушка: логика — packet-04.
pub fn decide(_req: &CallRequest) -> Verdict {
    unimplemented!("M3/packet-04: решение по вызову")
}
