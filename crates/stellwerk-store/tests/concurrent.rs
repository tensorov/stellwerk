//! RED-спецификация: конкурентная запись событий.
//!
//! Федерация пишет события из нескольких воркеров одновременно. Если `append`
//! теряет записи или падает под конкуренцией, `watch` покажет неполную картину,
//! а gate посчитает цепочку доказательств неверно. Поэтому проверяется именно
//! **счётчик**: сколько записали — столько и прочитали.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use stellwerk_core::contract::{Event, EventRisk, EventSource, RiskClass};
use stellwerk_store::{EventStore, SqliteStore};

const THREADS: usize = 10;
const PER_THREAD: usize = 10;

fn store() -> Arc<SqliteStore> {
    Arc::new(SqliteStore::open(Path::new(":memory:")).expect("store must open"))
}

fn event(thread: usize, n: usize) -> Event {
    let mut payload = BTreeMap::new();
    payload.insert("thread".to_owned(), thread.to_string());
    payload.insert("n".to_owned(), n.to_string());
    Event {
        id: format!("evt-t{thread}-n{n}"),
        source: EventSource::System,
        event_type: "run.step".to_owned(),
        actor: format!("worker-{thread}"),
        workspace: Some("ws-stellwerk".to_owned()),
        risk: EventRisk::Classified(RiskClass::R1),
        payload,
        // Разные секунды, чтобы проверка порядка не зависела от interleaving.
        ts: format!("2026-09-27T01:{:02}:{:02}Z", thread, n),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ten_writers_append_all_events_and_count_is_exact() {
    let store = store();
    let mut handles = Vec::new();

    for t in 0..THREADS {
        let store = Arc::clone(&store);
        handles.push(tokio::spawn(async move {
            for n in 0..PER_THREAD {
                store.append(event(t, n)).await?;
            }
            Ok::<(), stellwerk_store::StoreError>(())
        }));
    }

    let mut succeeded = 0usize;
    for handle in handles {
        match handle.await {
            Ok(Ok(())) => succeeded += 1,
            Ok(Err(err)) => panic!("поток завершился ошибкой записи: {err}"),
            Err(join) => panic!("поток упал: {join}"),
        }
    }
    assert_eq!(
        succeeded, THREADS,
        "все {THREADS} писателей обязаны завершиться"
    );

    let all = store
        .list_since("2026-09-27T00:00:00Z")
        .await
        .expect("list_since must succeed");
    assert_eq!(
        all.len(),
        THREADS * PER_THREAD,
        "ни одна запись не должна потеряться под конкуренцией"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_writers_produce_unique_ids() {
    // Потерянная под блокировкой запись проявила бы себя и так: id обязаны быть
    // уникальны, иначе посчитанная длина соврала бы.
    let store = store();
    let mut handles = Vec::new();
    for t in 0..THREADS {
        let store = Arc::clone(&store);
        handles.push(tokio::spawn(async move {
            for n in 0..PER_THREAD {
                store.append(event(t, n)).await?;
            }
            Ok::<(), stellwerk_store::StoreError>(())
        }));
    }
    for handle in handles {
        let _ = handle.await.expect("writer must not panic");
    }

    let all = store
        .list_since("2026-09-27T00:00:00Z")
        .await
        .expect("list");
    let mut ids: Vec<&str> = all.iter().map(|e| e.id.as_str()).collect();
    let before = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), before, "идентификаторы обязаны быть уникальными");
}

/// События одной задачи при конкурентной записи не должны теряться: это ровно
/// тот случай, из-за которого в схеме есть колонка `task_id` и индекс по ней.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_appends_to_one_task_are_all_listed() {
    let store = store();
    let mut handles = Vec::new();
    for t in 0..THREADS {
        let store = Arc::clone(&store);
        handles.push(tokio::spawn(async move {
            for n in 0..PER_THREAD {
                let mut ev = event(t, n);
                ev.payload.insert("task_id".to_owned(), "t-1".to_owned());
                store.append(ev).await?;
            }
            Ok::<(), stellwerk_store::StoreError>(())
        }));
    }
    for handle in handles {
        let _ = handle.await.expect("writer must not panic");
    }

    let for_task = store
        .list_by_task("t-1")
        .await
        .expect("list_by_task must succeed");
    assert_eq!(for_task.len(), THREADS * PER_THREAD);
}
