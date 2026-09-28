//! RED-спецификация: продвижение статуса задачи через предикаты контракта.
//!
//! Запрещённый переход обязан быть отвергнут, а разрешённый — дойти до
//! хранилища: `advance`, вернувшая `Planned` и ничего не изменив, исполнила бы
//! работу наполовину и выглядела бы исправной.
//!
//! Пять сценариев — предикаты `TaskStatus::can_transition_to` (пакет M1), зелёные
//! сразу. Два последних зовут `advance`: Given `Draft`, When просят `Planned`,
//! Then переход разрешён И задача числится в `Planned` по `list_by_status`; Given
//! `Done`, When просят `Running`, Then отказ `IllegalTransition` И статус прежний.
//! Оба падают: тело ещё `unimplemented!()`.
//!
//! Импорт через публичный API крейта, не через `#[path]`: тот скомпилил бы вторую
//! копию модуля и оставил экспорт непроверенным (F-14).

use std::path::Path;

use stellwerk_core::contract::{DataClass, TaskLimits, TaskSpec, TaskStatus};
use stellwerk_store::{SqliteStore, TaskStore};
use stellwerk_task::lifecycle::{advance, LifecycleError};

fn spec(id: &str) -> TaskSpec {
    TaskSpec {
        id: id.to_owned(),
        goal: "Реализовать cmd_measure".to_owned(),
        workspace_ref: "ws-stellwerk".to_owned(),
        model_ref: "qwen-local".to_owned(),
        packet: "сделай ровно одну вещь".to_owned(),
        limits: TaskLimits {
            max_files: 3,
            max_diff_lines: 200,
            timeout_seconds: 900,
            max_retries: 2,
            context_budget_tokens: 40_000,
            data_class: DataClass::Internal,
        },
        evidence_policy: "cargo test -p stellwerk-core".to_owned(),
    }
}

/// `:memory:` — база живёт ровно столько, сколько нужно тесту.
fn store() -> SqliteStore {
    SqliteStore::open(Path::new(":memory:")).expect("store must open")
}

/// `TaskStore` отдаёт future, а тесты синхронны: `advance` же зовётся вне
/// рантайма — см. её docs.
fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Runtime::new().expect("runtime must build")
}

/// `create` кладёт `Draft`, дальше — прямая запись в заданный статус.
async fn task_in(store: &SqliteStore, id: &str, status: TaskStatus) {
    store.create(spec(id)).await.expect("create must succeed");
    store
        .update_status(id, status)
        .await
        .expect("update must succeed");
}

#[test]
fn contract_allows_draft_to_planned() {
    assert!(TaskStatus::Draft.can_transition_to(TaskStatus::Planned));
}

#[test]
fn contract_forbids_draft_to_verified() {
    assert!(!TaskStatus::Draft.can_transition_to(TaskStatus::Verified));
}

#[test]
fn contract_allows_running_suspended_roundtrip() {
    assert!(TaskStatus::Running.can_transition_to(TaskStatus::Suspended));
    assert!(TaskStatus::Suspended.can_transition_to(TaskStatus::Running));
}

#[test]
fn contract_forbids_done_to_running() {
    assert!(!TaskStatus::Done.can_transition_to(TaskStatus::Running));
}

#[test]
fn contract_allows_verified_to_validated_but_not_back_to_running() {
    assert!(TaskStatus::Verified.can_transition_to(TaskStatus::Validated));
    assert!(!TaskStatus::Validated.can_transition_to(TaskStatus::Running));
}

#[test]
fn advance_writes_new_status_into_store() {
    let store = store();
    let rt = rt();
    rt.block_on(task_in(&store, "t-1", TaskStatus::Draft));

    let landed = advance(&store, "t-1", TaskStatus::Planned);

    assert_eq!(
        landed.expect("переход разрешён контрактом"),
        TaskStatus::Planned
    );
    let planned = rt
        .block_on(store.list_by_status(TaskStatus::Planned))
        .expect("выборка обязана читаться");
    assert!(
        planned.iter().any(|t| t.id == "t-1"),
        "задача обязана числиться в Planned, получено {planned:?}"
    );
}

#[test]
fn forbidden_advance_leaves_status_untouched() {
    let store = store();
    let rt = rt();
    rt.block_on(task_in(&store, "t-2", TaskStatus::Done));

    let refused = advance(&store, "t-2", TaskStatus::Running);

    assert!(
        matches!(refused, Err(LifecycleError::IllegalTransition { .. })),
        "Done -> Running обязан быть IllegalTransition, получено {refused:?}"
    );
    let after = rt
        .block_on(store.status_of("t-2"))
        .expect("чтение обязано удаться");
    assert_eq!(
        after,
        Some(TaskStatus::Done),
        "запрещённый переход обязан оставить статус прежним"
    );
}
