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
    /// Реализация ещё не добавлена.
    #[error("permission check not implemented")]
    NotImplemented,
}

// ═══════════════════════════════════════════════════════════════════════════
// permission_for
// ═══════════════════════════════════════════════════════════════════════════

/// Определить разрешение для инструмента по его риску.
///
/// RED-заглушка — вердикт всегда `NotImplemented`.
/// Реализация будет в packet-02.
pub fn permission_for(_risk: ToolRisk) -> Result<Permission, PermissionError> {
    Err(PermissionError::NotImplemented)
}
