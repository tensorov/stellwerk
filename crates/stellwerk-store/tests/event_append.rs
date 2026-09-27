//! RED-спецификация: поток событий.
//!
//! События — append-only, поэтому главное свойство не «можно записать», а
//! «можно досчитать поток с момента». `list_since` подписывает UI на дельту;
//! ошибка на границе (включён ли `ts` из аргумента) ломает и watch, и outbox.

use std::collections::BTreeMap;
use std::path::Path;

use stellwerk_core::contract::{Event, EventSource, RiskClass};
use stellwerk_store::{EventStore, SqliteStore};

fn store() -> SqliteStore {
    SqliteStore::open(Path::new(":memory:")).expect("store must open")
}

/// Время события с индексом: 100 событий укладываются в 0..99 секунд.
fn ts(i: usize) -> String {
    format!("2026-09-27T00:{:02}:{:02}Z", i / 60, i % 60)
}

fn event(i: usize) -> Event {
    let mut payload = BTreeMap::new();
    payload.insert("n".to_owned(), i.to_string());
    Event {
        id: format!("evt-{i:04}"),
        source: EventSource::System,
        event_type: "run.step".to_owned(),
        actor: "worker-1".to_owned(),
        workspace: Some("ws-stellwerk".to_owned()),
        risk: RiskClass::R1,
        payload,
        ts: ts(i),
    }
}

#[tokio::test]
async fn append_100_then_list_since_50th_returns_50() {
    let store = store();
    for i in 0..100 {
        store.append(event(i)).await.expect("append must succeed");
    }

    let tail = store
        .list_since(&ts(50))
        .await
        .expect("list_since must succeed");

    assert_eq!(tail.len(), 50, "с 50-го по 99-й включительно — ровно 50 событий");
    assert_eq!(tail.first().unwrap().id, "evt-0050", "граница включительная");
    assert_eq!(tail.last().unwrap().id, "evt-0099");
}

#[tokio::test]
async fn list_since_returns_events_in_time_order() {
    let store = store();
    // Пишем задом наперёд: поток обязан вернуться отсортированным по времени.
    for i in (0..10).rev() {
        store.append(event(i)).await.expect("append must succeed");
    }
    let all = store.list_since(&ts(0)).await.expect("list must succeed");
    let times: Vec<&str> = all.iter().map(|e| e.ts.as_str()).collect();
    let mut sorted = times.clone();
    sorted.sort_unstable();
    assert_eq!(times, sorted, "поток обязан быть упорядочен по ts");
}

#[tokio::test]
async fn list_by_task_filters_and_keeps_others_out() {
    let store = store();
    let mut own = event(1);
    own.task_in_payload("t-1");
    let mut foreign = event(2);
    foreign.task_in_payload("t-2");
    store.append(own).await.expect("append own");
    store.append(foreign).await.expect("append foreign");

    let mine = store.list_by_task("t-1").await.expect("list must succeed");
    assert_eq!(mine.len(), 1, "события чужой задачи не должны попадать в выборку");
    assert_eq!(mine[0].id, "evt-0001");
}

#[tokio::test]
async fn append_preserves_payload() {
    let store = store();
    let original = event(7);
    store.append(original.clone()).await.expect("append must succeed");
    let all = store.list_since(&ts(0)).await.expect("list must succeed");
    assert_eq!(all[0], original, "событие обязано пережить roundtrip без потерь");
}

/// Маленький helper, чтобы тест выше не расползался индексами payload.
trait TaskInPayload {
    fn task_in_payload(&mut self, task_id: &str);
}

impl TaskInPayload for Event {
    fn task_in_payload(&mut self, task_id: &str) {
        self.payload.insert("task_id".to_owned(), task_id.to_owned());
    }
}
