//! RED-спецификация: хранилище модулей реестра.
//!
//! Ключевое свойство — **идемпотентность upsert**: `module measure` переписывает
//! запись при каждом замере, и реестр не должен ни плодить дубликаты, ни терять
//! прежние поля. Поэтому тест проверяет именно обновление, а не только вставку.

use std::path::Path;

use stellwerk_core::contract::{ModuleKind, RiskClass, Source};
use stellwerk_store::{ModuleRecord, ModuleStore, SqliteStore};

fn store() -> SqliteStore {
    SqliteStore::open(Path::new(":memory:")).expect("store must open")
}

fn record(name: &str, cost: u64) -> ModuleRecord {
    ModuleRecord {
        name: name.to_owned(),
        kind: ModuleKind::Mcp,
        risk: Some(RiskClass::R3),
        source: Source::Git {
            git: "https://github.com/example/polyforge".to_owned(),
            rev: "abc123".to_owned(),
        },
        command: Some("/run/polyforge".to_owned()),
        tools: vec!["read".to_owned(), "write".to_owned()],
        context_cost_tokens: cost,
    }
}

#[tokio::test]
async fn upsert_then_get_returns_record() {
    let store = store();
    store
        .upsert_lock_entry(record("polyforge", 813))
        .await
        .expect("upsert must succeed");

    let loaded = store.get("polyforge").await.expect("get must succeed");
    let loaded = loaded.expect("модуль обязан существовать после upsert");
    assert_eq!(loaded.name, "polyforge");
    assert_eq!(loaded.kind, ModuleKind::Mcp);
    assert_eq!(loaded.risk, Some(RiskClass::R3));
    assert_eq!(loaded.context_cost_tokens, 813);
}

#[tokio::test]
async fn second_upsert_updates_in_place() {
    let store = store();
    store
        .upsert_lock_entry(record("polyforge", 813))
        .await
        .expect("first upsert");
    store
        .upsert_lock_entry(record("polyforge", 1500))
        .await
        .expect("second upsert must succeed");

    let loaded = store.get("polyforge").await.expect("get must succeed");
    let loaded = loaded.expect("модуль обязан существовать");
    assert_eq!(
        loaded.context_cost_tokens, 1500,
        "повторный upsert обязан обновлять цену, а не игнорировать её"
    );
}

#[tokio::test]
async fn upsert_twice_does_not_duplicate() {
    let store = store();
    store
        .upsert_lock_entry(record("polyforge", 813))
        .await
        .expect("upsert 1");
    store
        .upsert_lock_entry(record("polyforge", 900))
        .await
        .expect("upsert 2");

    let all = store.list().await.expect("list must succeed");
    let matching: Vec<_> = all.iter().filter(|m| m.name == "polyforge").collect();
    assert_eq!(
        matching.len(),
        1,
        "имя модуля — ключ: дубликат означает двойной учёт цены"
    );
}

#[tokio::test]
async fn get_unknown_module_is_none() {
    let store = store();
    assert_eq!(
        store.get("нет-такого").await.expect("get must succeed"),
        None
    );
}

#[tokio::test]
async fn list_returns_all_modules_sorted_by_name() {
    let store = store();
    for name in ["zebra", "alpha", "mike"] {
        store
            .upsert_lock_entry(record(name, 10))
            .await
            .expect("upsert must succeed");
    }
    let all = store.list().await.expect("list must succeed");
    let names: Vec<&str> = all.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(
        names,
        ["alpha", "mike", "zebra"],
        "порядок обязан быть стабильным"
    );
}

#[tokio::test]
async fn roundtrip_preserves_source_variant() {
    let store = store();
    let mut npm = record("node-tool", 42);
    npm.kind = ModuleKind::Skill;
    npm.source = Source::Npm {
        npm: "@scope/pkg".to_owned(),
        version: Some("1.2.3".to_owned()),
    };
    npm.risk = None;
    store
        .upsert_lock_entry(npm.clone())
        .await
        .expect("upsert must succeed");

    let loaded = store
        .get("node-tool")
        .await
        .expect("get")
        .expect("must exist");
    assert_eq!(
        loaded.source, npm.source,
        "вариант источника обязан пережить JSON"
    );
    assert_eq!(loaded.kind, ModuleKind::Skill);
    assert_eq!(loaded.risk, None);
}

#[tokio::test]
async fn roundtrip_preserves_crate_variant() {
    let store = store();
    let mut krate = record("rust-tool", 7);
    krate.source = Source::Crate {
        krate: "serde".to_owned(),
        version: None,
    };
    store
        .upsert_lock_entry(krate.clone())
        .await
        .expect("upsert must succeed");
    let loaded = store
        .get("rust-tool")
        .await
        .expect("get")
        .expect("must exist");
    assert_eq!(loaded.source, krate.source);
}
