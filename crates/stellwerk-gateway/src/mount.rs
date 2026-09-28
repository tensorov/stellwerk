//! Ленивый монтаж модулей: бюджет контекста решает, что вообще подключать.
//!
//! Отбор жадный, по возрастанию `cost_tokens`: дешёвые базовые инструменты нужны
//! больше, а порядок не зависит от порядка входа — иначе две сессии с одним
//! набором модулей получили бы разные бюджеты. Монтаж решает, что ВЛЕЗАЕТ в
//! контекст, а не что РАЗРЕШЕНО: по риску отсекается только R5, а `Ask` у R3/R4
//! разбирается в момент вызова, в `decide` (В1, В4).
//!
//! GREEN M3/packet-08: жадный проход по возрастанию цены, отсев по риску.

use std::collections::HashSet;

use thiserror::Error;

use stellwerk_core::contract::{ModuleKind, RiskClass};

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
    /// «дорого» в расследовании — разные вещи. Порядок — порядок жадного прохода
    /// (по возрастанию `cost_tokens`), а не порядок входа.
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

/// Смонтировать то, что влезает в бюджет: жадный проход по возрастанию цены.
///
/// Порядок входа не влияет на результат — иначе две сессии с одним набором
/// модулей получили бы разные бюджеты, а разница стала бы неразличимой от смены
/// состава. Имя вторым полем сортировки делает проход детерминированным при
/// равных ценах.
///
/// Отказ от всего набора из-за одного дорогого модуля хуже, чем дешёвые
/// инструменты: невлезение — это `OverBudget` и продолжение прохода.
///
/// Порядок проверок — вид, риск, повтор имени, бюджет: первые три от накопленного
/// не зависят, а бюджет — единственное накопительное условие.
pub fn plan(candidates: &[Candidate], budget_tokens: u64) -> Result<MountPlan, MountError> {
    let mut ordered: Vec<&Candidate> = candidates.iter().collect();
    ordered.sort_by_key(|c| (c.cost_tokens, c.name.as_str()));

    let mut result = MountPlan {
        mounted: Vec::new(),
        skipped: Vec::new(),
    };
    // Имена смонтированных: повтор определяется составом, а не бюджетом, и
    // повтором считается только то имя, которое уже смонтировано.
    let mut mounted_names: HashSet<&str> = HashSet::with_capacity(ordered.len());
    // Потрачено токенов контекста. Насыщение вместо переполнения: бюджет приходит
    // извне, и `u64::MAX + 1` — не повод паниковать.
    let mut spent: u64 = 0;

    for candidate in ordered {
        // Имя копируется один раз: любая ветка кладёт его либо в `mounted`, либо в
        // `skipped`, а результат должен пережить `candidates`.
        let name = candidate.name.clone();
        match candidate.kind {
            // Skill — инструкция: инструментов не даёт, цену контекста платит.
            ModuleKind::Skill => {
                result.skipped.push((name, SkipReason::Disabled));
                continue;
            }
            ModuleKind::Mcp => {}
        }

        // В2: неизвестный риск — состояние, а не R0, и отсекается он ДО
        // арифметики: «неизвестно плюс минус один» не имеет смысла. Прежний
        // `unwrap_or(R0)` превращал бы отсутствие данных в разрешение.
        let class = match candidate.risk {
            ToolRisk::Classified(class) => class,
            ToolRisk::Unclassified => {
                result.skipped.push((name, SkipReason::Unclassified));
                continue;
            }
        };

        // Отсек по риску — только R5. Прежнее «выше R2 → TooRisky» делало решения
        // В1/В4 мёртвым кодом: R3/R4 монтируются, их `Ask` разбирается в `decide`.
        if hits_risk_ceiling(class, candidate.risk_delta) {
            result.skipped.push((name, SkipReason::TooRisky));
            continue;
        }

        if mounted_names.contains(candidate.name.as_str()) {
            result.skipped.push((name, SkipReason::Duplicate));
            continue;
        }

        // Инвариант бюджета держится насыщением и целочисленным сложением: `u64`
        // цены не смешивается со знаковым сдвигом риска.
        let next = spent.saturating_add(candidate.cost_tokens);
        if next > budget_tokens {
            result.skipped.push((name, SkipReason::OverBudget));
            continue;
        }

        spent = next;
        mounted_names.insert(candidate.name.as_str());
        result.mounted.push(name);
    }

    Ok(result)
}

/// Порядковый номер R5 — единственного класса, отсекающего монтаж.
const RISK_CEILING_RANK: i64 = 5;

/// Пересекает ли сдвиг бюджета сессии потолок риска.
///
/// Считается сравнением, а не материализацией класса: единственный вопрос монтажа
/// — «это R5 или нет», и класс после сдвива больше нигде не нужен. Порядковые
/// номера выписаны match'ем, а не выведены из `Ord`: появление R6 заставит
/// компилятор потребовать решения здесь, как и в `permission_for_class`.
///
/// Насыщение, а не переполнение: `risk_delta = i64::MAX` — это «очень высоко», а
/// не паника в отладке. Нижняя граница не нужна — вопрос только про верхнюю.
fn hits_risk_ceiling(class: RiskClass, delta: i64) -> bool {
    let rank = match class {
        RiskClass::R0 => 0,
        RiskClass::R1 => 1,
        RiskClass::R2 => 2,
        RiskClass::R3 => 3,
        RiskClass::R4 => 4,
        RiskClass::R5 => RISK_CEILING_RANK,
    };
    rank.saturating_add(delta) >= RISK_CEILING_RANK
}
