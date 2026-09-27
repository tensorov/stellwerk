//! Контракт события должен пережить JSON-цикл без потерь.
//! GREEN уже: типы чисто сериализуются. Проверяет, что форма события не поплывёт
//! при смене транспорта (outbox → NATS → SQLite).

use std::collections::BTreeMap;

use serde_json::json;
use stellwerk_core::contract::{Event, EventSource, RiskClass};

fn event(source: EventSource) -> Event {
    let mut payload = BTreeMap::new();
    payload.insert("task_id".to_owned(), "t-1".to_owned());
    payload.insert("run_id".to_owned(), "r-9".to_owned());
    Event {
        id: "evt-0001".to_owned(),
        source,
        event_type: "task.delegated".to_owned(),
        actor: "owner".to_owned(),
        workspace: Some("ws-stellwerk".to_owned()),
        risk: RiskClass::R2,
        payload,
        ts: "2026-09-27T02:14:03Z".to_owned(),
    }
}

/// Каждый источник обязан пережить roundtrip: список источников расширяется, и
/// забытый вариант молча потерял бы данные при синхронизации.
#[test]
fn event_roundtrip_for_every_source() {
    let sources = [
        EventSource::Manual,
        EventSource::OpenCode,
        EventSource::Kanban,
        EventSource::Startup,
        EventSource::Calendar,
        EventSource::Hermes,
        EventSource::System,
    ];
    for source in sources {
        let original = event(source);
        let json = serde_json::to_string(&original).expect("event must serialize");
        let restored: Event = serde_json::from_str(&json).expect("event must deserialize");
        assert_eq!(original, restored, "roundtrip broken for {source:?}");
    }
}

#[test]
fn event_source_serialization_is_snake_case() {
    assert_eq!(
        serde_json::to_value(EventSource::OpenCode).unwrap(),
        json!("open_code")
    );
    assert_eq!(
        serde_json::to_value(EventSource::Manual).unwrap(),
        json!("manual")
    );
}

#[test]
fn event_risk_class_serializes_as_r_level() {
    assert_eq!(serde_json::to_value(RiskClass::R3).unwrap(), json!("R3"));
    assert_eq!(serde_json::to_value(RiskClass::R0).unwrap(), json!("R0"));
}

/// `workspace: None` — федеративное событие, не ошибка. Поле обязано быть
/// необязательным и переживать roundtrip.
#[test]
fn event_without_workspace_roundtrips() {
    let mut original = event(EventSource::System);
    original.workspace = None;
    let json = serde_json::to_string(&original).unwrap();
    let restored: Event = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.workspace, None);
}

/// Пустая payload обязана быть допустима: не каждое событие несут данные.
#[test]
fn event_with_empty_payload_roundtrips() {
    let mut original = event(EventSource::Startup);
    original.payload = BTreeMap::new();
    let json = serde_json::to_string(&original).unwrap();
    let restored: Event = serde_json::from_str(&json).unwrap();
    assert!(restored.payload.is_empty());
}

/// `deny_unknown_fields` на Event: событие из будущей версии не должно молча
/// терять поля при разборе старым читателем.
#[test]
fn event_rejects_unknown_fields() {
    let bad = json!({
        "id": "evt-1", "source": "manual", "event_type": "x", "actor": "a",
        "workspace": null, "risk": "R0", "payload": {}, "ts": "t", "extra": 1
    });
    assert!(serde_json::from_value::<Event>(bad).is_err());
}
