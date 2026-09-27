//! Итоговое решение по вызову: риск инструмента, полномочия вызывающего и класс
//! данных сводятся в один `Verdict`.
//!
//! Три независимых правила, порядок обязателен: риск инструмента, полномочия
//! вызывающего, класс данных. `Deny` по любому из них возвращается немедленно;
//! иначе `Ask` при R2/R3/R4, иначе `Allow`.
//!
//! Правила принимаются ТОЛЬКО через предикаты контракта и `permission_for`:
//! своей таблицы рисков и своих уровней здесь нет.

use crate::permission::{permission_for, Permission, ToolRisk};
use stellwerk_core::contract::{Action, DataClass, ModelBinding, PolicyLevel};

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

/// Итоговый вердикт по вызову.
///
/// Окно ответа — 24ч, для R4 — 4ч, и его истечение даёт `Failed(approval_timeout)`,
/// а не `Deny` (В4). Отсчёт живёт в пакете inbox/approval: у `decide` нет часов,
/// а выдуманное время решило бы судьбу прогона, не спросив никого.
pub fn decide(req: &CallRequest) -> Verdict {
    // Правило 1 — риск инструмента, через `permission_for`: своей таблицы нет.
    let permission = match permission_for(req.risk) {
        Ok(permission) => permission,
        Err(err) => return deny(format!("risk_table_incomplete: {err}")),
    };

    let needs_approval = match req.risk {
        // В2: `Unclassified` — отсутствие данных, а не нулевой риск. Спросить про
        // неизвестный инструмент вправе только владелец политики, и лишь когда
        // спросить реально: без интерактива «пока подумаем» неотличимо от
        // разрешения, а `Suspended` неуместен — вопрос задать некому.
        ToolRisk::Unclassified => {
            if !req.interactive {
                return deny(
                    "unclassified_tool: риск инструмента не определён, а спросить некого — классифицируйте тул в modules.toml",
                );
            }
            if !req.caller.permits(Action::GrantRoles) {
                return deny(
                    "unclassified_tool: риск инструмента не определён; ответить про него может только владелец политики",
                );
            }
            true
        }
        ToolRisk::Classified(class) => {
            // R5: необратимое разрушение не делегируется агенту ни при каком
            // уровне, включая L5Owner. Отказ приходит из таблицы до `Ask`, поэтому
            // R5 не спрашивается и не suspend'ится: владелец делает force-push руками.
            if permission == Permission::Deny {
                return deny(format!(
                    "risk_denied: класс {class:?} запрещён таблицей RISK_PERMISSION"
                ));
            }
            // Уровень берётся только из контракта (ADR-0002: R3 и R4 требуют
            // L5Owner). Нехватка уровня отказывает ДО Ask: у L2Contributor нельзя
            // спросить разрешение на сетевой выход, его принимает тот, у кого право есть.
            if req.caller < class.required_level() {
                return deny(format!(
                    "insufficient_level: {:?} ниже требуемого {:?}",
                    req.caller,
                    class.required_level()
                ));
            }
            permission == Permission::Ask
        }
    };

    // Правило 3 — данные, единственным предикатом `permits_model`. Binding
    // обязателен в типе, поэтому «привязки нет» не представимо: уйти безымянно
    // данные не могут.
    if !req.data.permits_model(&req.model) {
        return deny(format!(
            "data_class_exceeds_model: {:?} выше, чем держит binding {:?} ({:?})",
            req.data, req.model.provider, req.model.retention
        ));
    }

    if !needs_approval {
        return Verdict::Allow;
    }
    // В4: `Ask` без интерактива — `Suspended`, а не `Deny`: «никто не ответил» и
    // «запрещено политикой» обязаны различаться в статусе и в evidence.
    if req.interactive {
        Verdict::Ask
    } else {
        Verdict::Suspended
    }
}

/// `Deny` с обязательной причиной: пустая строка не сказала бы агенту, что делать.
fn deny(reason: impl Into<String>) -> Verdict {
    Verdict::Deny {
        reason: reason.into(),
    }
}
