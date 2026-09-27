//! Permission decisions for the gateway.
//!
//! `permission_for` принимает `ToolRisk` и возвращает `Permission` через
//! предикаты контракта. Своя копия правил запрещена.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use stellwerk_core::contract::RiskClass;

// ═══════════════════════════════════════════════════════════════════════════
// Permission
// ═══════════════════════════════════════════════════════════════════════════

/// Вердикт gateway'а для инструмента.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Permission {
    /// Разрешено без вопросов.
    Allow,
    /// Спросить человека.
    Ask,
    /// Запрещено.
    Deny,
}

// ═══════════════════════════════════════════════════════════════════════════
// ToolRisk
// ═══════════════════════════════════════════════════════════════════════════

/// Класс риска инструмента, известный gateway'у.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolRisk {
    /// Риск известен и определён контрактом.
    Classified(RiskClass),
    /// Риск не определён (неизвестный инструмент).
    Unclassified,
}

// ═══════════════════════════════════════════════════════════════════════════
// PermissionError
// ═══════════════════════════════════════════════════════════════════════════

/// Ошибка разрешения.
#[derive(Debug, Error)]
pub enum PermissionError {
    /// Класс риска есть в контракте, но отсутствует в `RISK_PERMISSION`.
    ///
    /// Это дефект крейта, а не условие предметной области. Молчаливый `Deny`
    /// здесь был бы не осторожностью, а сокрытием дыры: по статусу он
    /// неотличим от осознанного отказа.
    #[error("risk {risk:?} отсутствует в таблице RISK_PERMISSION")]
    TableIncomplete { risk: RiskClass },
}

/// Таблица «класс риска → вердикт». Единственный источник правды в крейте:
/// её читают и `permission_for`, и будущий тест полноты.
///
/// Уровни здесь намеренно НЕ продублированы: столбец уровня живёт в контракте
/// (`RiskClass::required_level`, ADR-0002). Своя копия уровней в gateway была бы
/// вторым источником истины ровно того класса, который крейт существует чтобы
/// убрать.
pub const RISK_PERMISSION: [(RiskClass, Permission); 6] = [
    (RiskClass::R0, Permission::Allow),
    (RiskClass::R1, Permission::Allow),
    (RiskClass::R2, Permission::Ask),
    (RiskClass::R3, Permission::Ask),
    // В1: R4 — это L5Owner + Ask на ОДНО действие. Ветка «L4Admin и ниже →
    // Deny» здесь не выражается: у `permission_for` нет параметра «уровень
    // сессии». См. FINDING о packet-02.
    (RiskClass::R4, Permission::Ask),
    // В1: R5 — безусловный Deny при любом уровне, включая L5Owner. Владелец
    // делает `push --force` руками, не через агента.
    (RiskClass::R5, Permission::Deny),
];

// ═══════════════════════════════════════════════════════════════════════════
// permission_for
// ═══════════════════════════════════════════════════════════════════════════

/// Определить разрешение для инструмента по его риску.
pub fn permission_for(risk: ToolRisk) -> Result<Permission, PermissionError> {
    match risk {
        // В2: отсутствие данных — отдельное состояние, а не R0. Здесь оно всегда
        // `Deny`: Ask при `Unclassified` допустим лишь в интерактивной сессии
        // L5Owner, а признака интерактивности у этой функции нет. Выбирать Ask
        // по умолчанию значило бы разрешить то, о чём никто не высказался.
        ToolRisk::Unclassified => Ok(Permission::Deny),
        ToolRisk::Classified(class) => permission_for_class(class),
    }
}

fn lookup(risk: RiskClass) -> Result<Permission, PermissionError> {
    RISK_PERMISSION
        .iter()
        .find(|(k, _)| *k == risk)
        .map(|(_, verdict)| *verdict)
        .ok_or(PermissionError::TableIncomplete { risk })
}

fn permission_for_class(class: RiskClass) -> Result<Permission, PermissionError> {
    // Исчерпывающий match: появление R7 заставит компилятор потребовать решения
    // здесь. Ветви обязаны оставаться делегирующими (`R0 => lookup(R0)`), а не
    // тождественными: свернутые в `class` clippy справедливо назовёт лишними, и
    // следующий `clippy --fix` снесёт match вместе с этой гарантией.
    match class {
        RiskClass::R0 => lookup(RiskClass::R0),
        RiskClass::R1 => lookup(RiskClass::R1),
        RiskClass::R2 => lookup(RiskClass::R2),
        RiskClass::R3 => lookup(RiskClass::R3),
        RiskClass::R4 => lookup(RiskClass::R4),
        RiskClass::R5 => lookup(RiskClass::R5),
    }
}
