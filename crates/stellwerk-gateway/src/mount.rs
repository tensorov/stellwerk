//! Ленивый монтаж модулей: бюджет контекста решает, что вообще подключать.
//!
//! Отбор жадный, по возрастанию `cost_tokens`: дешёвые базовые инструменты нужны
//! больше, а порядок не зависит от порядка входа — иначе две сессии с одним
//! набором модулей получили бы разные бюджеты. Монтаж решает, что ВЛЕЗАЕТ в
//! контекст, а не что РАЗРЕШЕНО: по риску отсекается только R5, а `Ask` у R3/R4
//! разбирается в момент вызова, в `decide` (В1, В4).
//!
//! RED-заглушка M3/packet-07: поверхность объявлена, поведение — packet-08.

use thiserror::Error;

use stellwerk_core::contract::ModuleKind;

/// Риск модуля: `Classified(RiskClass)` или `Unclassified` (В2), а НЕ
/// `Option<RiskClass>` — `None` молча превращается в `R0`, то есть отсутствие
/// данных становится разрешением. Реэкспорт из `permission`, а не второе
/// объявление: риск при монтаже и риск в `decide` обязаны быть одним типом.
pub use crate::permission::ToolRisk;

/// Модуль-кандидат: всё, что известно о нём до решения о монтаже.
#[derive(Debug, Clone)]
pub struct Candidate {
    /// Имя модуля: по нему же ловится повтор.
    pub name: String,
    /// Вид модуля из `modules.toml`.
    pub kind: ModuleKind,
    /// Цена в токенах: число из `measure` (`context_cost_tokens`).
    pub cost_tokens: u64,
    /// Риск. `Unclassified` — состояние, а не нулевой риск.
    pub risk: ToolRisk,
    /// Сдвиг бюджета риска сессии: PolyForge −1 (СНИМАЕТ риск), сетевой MCP +1.
    pub risk_delta: i64,
}

/// Что смонтировано и что отсеяно — с причиной.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountPlan {
    /// Имена смонтированных, по возрастанию `cost_tokens`.
    pub mounted: Vec<String>,
    /// Отсеянные: имя и причина. Причина обязательна: «не классифицирован» и
    /// «дорого» в расследовании — разные вещи.
    pub skipped: Vec<(String, SkipReason)>,
}

/// Почему модуль не смонтирован.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// Не влезает в остаток бюджета. Жадный проход продолжается.
    OverBudget,
    /// R5 — необратимое разрушение. Единственный отсек по риску.
    TooRisky,
    /// Имя уже смонтировано: повтор.
    Duplicate,
    /// `ModuleKind::Skill`: инструментов не даёт, цену контекста платит.
    Disabled,
    /// Риск не определён. НЕ `R0`: неизвестно не значит дёшево.
    Unclassified,
}

/// Сбой планирования. Вариантов нет: планирование не ходит во внешний мир.
#[derive(Debug, Error)]
pub enum MountError {}

/// Смонтировать то, что влезает в бюджет. RED: поведение — M3/packet-08.
pub fn plan(candidates: &[Candidate], budget_tokens: u64) -> Result<MountPlan, MountError> {
    let _ = (candidates, budget_tokens);
    unimplemented!("plan: реализация в M3/packet-08")
}
