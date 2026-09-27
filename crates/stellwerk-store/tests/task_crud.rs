//! RED-спецификация: CRUD задач.
//!
//! Путь проверяется целиком: create → get → update_status → list_by_status.
//! Смысл связки — не «каждый метод устроен правильно», а «жизненный цикл задачи
//! сохраняется и виден в выборках»: без этого федерация не может ни найти
//! задачу, ни построить очередь.

use std::path::Path;

use stellwerk_core::contract::{DataClass, TaskLimits, TaskSpec, TaskStatus};
use stellwerk_store::{SqliteStore, TaskStore};

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

fn store() -> SqliteStore {
    // `:memory:` — база живёт ровно столько, сколько нужно тесту.
    SqliteStore::open(Path::new(":memory:")).expect("store must open: миграции обязаны примениться")
}

#[tokio::test]
async fn create_then_get_returns_the_same_spec() {
    let store = store();
    let created = store.create(spec("t-1")).await.expect("create must succeed");
    assert_eq!(created.id, "t-1");

    let loaded = store.get("t-1").await.expect("get must succeed");
    assert_eq!(loaded, Some(spec("t-1")), "спецификация обязана пережить roundtrip");
}

#[tokio::test]
async fn get_unknown_id_is_none_not_error() {
    let store = store();
    assert_eq!(store.get("нет-такой").await.expect("get must succeed"), None);
}

#[tokio::test]
async fn update_status_changes_state_and_is_visible_in_listing() {
    let store = store();
    store.create(spec("t-1")).await.expect("create must succeed");

    let updated = store
        .update_status("t-1", TaskStatus::Running)
        .await
        .expect("update must succeed");
    // TaskSpec не несёт статус: статус — колонка таблицы, а не поле типа.
    // Проверка статуса идёт через выборку ниже, а update_status обязан вернуть
    // актуальную спецификацию.
    assert_eq!(updated.id, "t-1");

    let running = store
        .list_by_status(TaskStatus::Running)
        .await
        .expect("list must succeed");
    assert_eq!(running.len(), 1, "задача обязана появиться в выборке по статусу");
    assert_eq!(running[0].id, "t-1");

    let drafts = store
        .list_by_status(TaskStatus::Draft)
        .await
        .expect("list must succeed");
    assert!(drafts.is_empty(), "старая выборка обязана опустеть");
}

#[tokio::test]
async fn update_status_of_unknown_task_is_not_found() {
    let store = store();
    let err = store
        .update_status("нет-такой", TaskStatus::Running)
        .await
        .expect_err("обновление несуществующей задачи обязано быть ошибкой");
    assert!(matches!(err, stellwerk_store::StoreError::NotFound(_)), "получено: {err:?}");
}

#[tokio::test]
async fn list_by_workspace_groups_tasks_of_one_workspace() {
    let store = store();
    let mut a = spec("t-1");
    let mut b = spec("t-2");
    let mut c = spec("t-3");
    a.workspace_ref = "ws-a".to_owned();
    b.workspace_ref = "ws-a".to_owned();
    c.workspace_ref = "ws-b".to_owned();
    store.create(a).await.expect("create a");
    store.create(b).await.expect("create b");
    store.create(c).await.expect("create c");

    let for_a = store
        .list_by_workspace("ws-a")
        .await
        .expect("list must succeed");
    assert_eq!(for_a.len(), 2, "в ws-a ровно две задачи");
    let for_b = store
        .list_by_workspace("ws-b")
        .await
        .expect("list must succeed");
    assert_eq!(for_b.len(), 1);
}

#[tokio::test]
async fn create_twice_does_not_silently_overwrite() {
    let store = store();
    store.create(spec("t-1")).await.expect("first create");
    let mut other = spec("t-1");
    other.goal = "другая цель".to_owned();
    let res = store.create(other).await;
    assert!(
        res.is_err(),
        "повторный create с тем же id обязан быть конфликтом, а не тихой перезаписью"
    );
}

/// Открытие базы само по себе обязано работать: миграции применяются при первом
/// подключении. Этот тест GREEN уже и служит доказательством, что схема
/// действительно создаётся, а не только описана в файлах.
#[tokio::test]
async fn open_applies_migrations_without_error() {
    let store = store();
    drop(store);
}
