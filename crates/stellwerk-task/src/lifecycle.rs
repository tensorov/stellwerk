//! # Продвижение статуса задачи
//!
//! `advance` — единственная точка, которой разрешено двигать задачу по
//! жизненному циклу. Легальность перехода решает **контракт**
//! (`TaskStatus::can_transition_to`): свой свод переходов разошёлся бы с ним при
//! первом же изменении.
//!
//! Текущее состояние задаёт **хранилище** (`status_of`), а не вызывающий код:
//! принять `from` от вызывающего — значило бы проверять вымышленное состояние, и
//! проверка была бы есть, а проверять было бы нечего.
//!
//! ## Отклонение от packet-05
//!
//! Параметр — `&impl TaskStore`, а не предписанный `&dyn TaskStore`: трейт
//! возвращает `impl Future + Send` (RPITIT) и потому не dyn-совместим
//! (`error[E0038]`), а починить это можно только правкой закрытого крейта store.
//! Аргументов по-прежнему три, `from` не появился.
//!
//! Три шага обязательны и идут именно в таком порядке: прочитать фактический
//! статус, спросить контракт, записать условно. `update_status` для переходов не
//! годится — он безусловный и не проверяет состояние.

use stellwerk_core::contract::TaskStatus;
use stellwerk_store::{StoreError, TaskStore};
use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════════
// Errors
// ═══════════════════════════════════════════════════════════════════════════

/// Отказ продвижения. Варианты различимы, потому что требуют разной реакции:
/// инфраструктура, нарушение контракта, проигранная гонка, запись без решения.
#[derive(Debug, Error)]
pub enum LifecycleError {
    /// Хранилище не прочитало или не записало состояние.
    #[error("store error: {0}")]
    Store(#[from] StoreError),
    /// Собственный рантайм `advance` не поднялся: блокировать нечем.
    #[error("cannot start runtime: {0}")]
    Runtime(#[from] std::io::Error),
    /// Контракт запрещает переход; `from` прочитан у задачи, не прислан.
    #[error("illegal transition {from:?} -> {to:?}")]
    IllegalTransition { from: TaskStatus, to: TaskStatus },
    /// Состояние сменил кто-то ещё; повтор стёр бы след чужого решения.
    #[error("state conflict: expected {expected:?}, refused to set {next:?}")]
    StateConflict {
        expected: TaskStatus,
        next: TaskStatus,
    },
    /// Запись состоялась как `Blocked` — запрос решения, а не успех.
    #[error("blocked: {0}")]
    Blocked(String),
}

// ═══════════════════════════════════════════════════════════════════════════
// Public API
// ═══════════════════════════════════════════════════════════════════════════

/// Продвинуть задачу `id` в статус `next` и вернуть фактически записанный
/// статус.
///
/// Запись обязана быть условной: между чтением и записью задачу может сменить
/// другой исполнитель, а проигранная гонка — `StateConflict`, не повтор.
/// Функция синхронна при async-хранилище: блокирует поток на своём рантайме и не
/// должна идти изнутри активного (`#[tokio::test]`).
pub fn advance(
    store: &impl TaskStore,
    id: &str,
    next: TaskStatus,
) -> Result<TaskStatus, LifecycleError> {
    let rt = tokio::runtime::Runtime::new()?;

    // Отсутствие задачи — `NotFound`, а не `Draft`: неизвестное не должно
    // выглядеть как значение по умолчанию.
    let found = rt.block_on(store.status_of(id))?;
    let Some(current) = found else {
        return Err(StoreError::NotFound(format!("task not found: {id}")).into());
    };

    if !current.can_transition_to(next) {
        return Err(LifecycleError::IllegalTransition {
            from: current,
            to: next,
        });
    }

    // Условие по статусу несёт сама запись: между чтением и записью состояние мог
    // сменить другой исполнитель. `false` — и несовпадение, и отсутствие задачи;
    // оба случая означают отказ, а повторная безусловная запись стёрла бы след
    // чужого решения.
    if !rt.block_on(store.compare_and_set_status(id, current, next))? {
        return Err(LifecycleError::StateConflict {
            expected: current,
            next,
        });
    }

    // Статус в базе уже `Blocked`, но это запрос решения, а не успех: вызывающий
    // обязан знать, что запись состоялась. Причина принадлежит вызывающему.
    if next == TaskStatus::Blocked {
        return Err(LifecycleError::Blocked(format!(
            "задача {id} заблокирована переходом {current:?} -> Blocked; причина — решение вызывающего"
        )));
    }

    Ok(next)
}
