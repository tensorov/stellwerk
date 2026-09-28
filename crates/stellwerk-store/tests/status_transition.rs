//! RED-спецификация: чтение фактического статуса и условная запись статуса.
//!
//! Статус живёт в колонке `tasks.status`, а прочитать его у ОДНОЙ задачи было
//! нечем: `TaskSpec` поля статуса не имеет, `list_by_status` — перебор 12
//! вариантов. Запись условная потому, что «прочитал → проверил → записал
//! безусловно» оставляет промежуток, в который задачу меняет другой исполнитель.

use std::path::Path;
use std::sync::Arc;

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

fn store() -> Arc<SqliteStore> {
    // `:memory:` — база живёт ровно столько, сколько нужно тесту.
    Arc::new(SqliteStore::open(Path::new(":memory:")).expect("store must open"))
}

/// `create` кладёт `Draft`, дальше — прямая запись в заданный статус.
async fn task_in(store: &SqliteStore, id: &str, status: TaskStatus) {
    store.create(spec(id)).await.expect("create must succeed");
    store
        .update_status(id, status)
        .await
        .expect("update_status must succeed");
}

#[tokio::test]
async fn status_of_returns_actual_stored_status() {
    let store = store();
    task_in(&store, "t-1", TaskStatus::Running).await;

    let status = store
        .status_of("t-1")
        .await
        .expect("status_of must succeed");
    assert_eq!(
        status,
        Some(TaskStatus::Running),
        "прочитанный статус обязан совпасть с записанным"
    );
}

#[tokio::test]
async fn status_of_unknown_id_is_none_not_default() {
    let store = store();

    let status = store
        .status_of("нет-такой")
        .await
        .expect("status_of must succeed");
    assert_eq!(status, None, "отсутствующая задача обязана дать None");
    assert_ne!(
        status,
        Some(TaskStatus::Draft),
        "отсутствие не должно выглядеть как Draft"
    );
}

#[tokio::test]
async fn compare_and_set_fails_when_expected_state_is_stale() {
    let store = store();
    task_in(&store, "t-1", TaskStatus::Draft).await;

    let won = store
        .compare_and_set_status("t-1", TaskStatus::Running, TaskStatus::Done)
        .await
        .expect("compare_and_set_status must succeed");
    assert!(!won, "устаревшее ожидаемое состояние обязано дать false");

    let after = store
        .status_of("t-1")
        .await
        .expect("status_of must succeed");
    assert_eq!(
        after,
        Some(TaskStatus::Draft),
        "неудачная условная запись обязана оставить статус прежним"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn exactly_one_of_two_compare_and_set_from_same_state_wins() {
    let store = store();
    task_in(&store, "t-1", TaskStatus::Draft).await;

    // Оба перехода бьют из одного `Draft` в разные цели: `Mutex<Connection>`
    // сериализует их, исход решает `WHERE status = ?` — порядок потоков не важен.
    let mut handles = Vec::new();
    for next in [TaskStatus::Running, TaskStatus::Suspended] {
        let store = Arc::clone(&store);
        handles.push(tokio::spawn(async move {
            store
                .compare_and_set_status("t-1", TaskStatus::Draft, next)
                .await
        }));
    }

    let mut winners = 0usize;
    for handle in handles {
        let won = handle
            .await
            .expect("переход не должен падать")
            .expect("переход не должен быть ошибкой хранилища");
        winners += usize::from(won);
    }
    assert_eq!(
        winners, 1,
        "ровно один из двух переходов из одного состояния обязан выиграть"
    );
}
