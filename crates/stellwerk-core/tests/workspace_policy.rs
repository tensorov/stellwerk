//! RED-спецификация: сетевая политика рабочего места.
//!
//! `NetworkPolicy::allows` объявлен `todo!()`. Инварианты, которые обязаны
//! выполняться после реализации:
//!   * `Deny` запрещает ЛЮБОЙ URL, включая пустую строку и «просто localhost»;
//!   * `AllowList` сравнивает хост точно, а не по префиксу (иначе
//!     `evil.example.com` прошёл бы под `example.com`);
//!   * `Allow` разрешает всё и требует явного решения владельца.

use stellwerk_core::contract::{NetworkPolicy, WorkspaceSpec};
use std::collections::BTreeMap;

fn workspace(policy: NetworkPolicy) -> WorkspaceSpec {
    WorkspaceSpec {
        id: "ws-1".to_owned(),
        base_commit: "033eb934c916ca2f1b3973126a44b2ecc8b3207a".to_owned(),
        repos: vec!["stellwerk".to_owned()],
        tools_allow: vec!["read".to_owned(), "edit".to_owned()],
        tools_deny: vec!["rm".to_owned()],
        mcp_servers: vec!["polyforge".to_owned()],
        network_policy: policy,
        env: BTreeMap::new(),
    }
}

/// Самое важное правило режима offline-first: воркер по умолчанию не ходит в сеть.
#[test]
fn deny_rejects_every_url() {
    let policy = NetworkPolicy::Deny;
    for url in [
        "",
        "https://example.com",
        "http://localhost:11434/v1",
        "https://registry.npmjs.org",
        "file:///etc/passwd",
        "not-a-url-at-all",
    ] {
        assert!(!policy.allows(url), "Deny обязан запрещать {url:?}");
    }
}

#[test]
fn allow_list_permits_only_listed_hosts() {
    let policy = NetworkPolicy::AllowList(vec![
        "example.com".to_owned(),
        "registry.npmjs.org".to_owned(),
    ]);
    assert!(policy.allows("https://example.com"));
    assert!(policy.allows("https://registry.npmjs.org/oh-my-pi"));
    for url in [
        "https://evil.example.com",
        "https://example.com.evil.net",
        "https://localhost",
        "",
    ] {
        assert!(!policy.allows(url), "AllowList обязан запрещать {url:?}");
    }
}

/// Пустой AllowList запрещает всё — как Deny. Иначе опечатка в конфиге
/// молча открыла бы сеть.
#[test]
fn empty_allow_list_denies_everything() {
    let policy = NetworkPolicy::AllowList(vec![]);
    assert!(!policy.allows("https://example.com"));
    assert!(!policy.allows(""));
}

#[test]
fn allow_permits_any_url() {
    let policy = NetworkPolicy::Allow;
    assert!(policy.allows("https://example.com"));
    assert!(policy.allows(""));
    assert!(policy.allows("not-a-url-at-all"));
}

#[test]
fn policy_is_inside_workspace_spec() {
    // Sanity: политика не потерялась при сборке спецификации.
    let ws = workspace(NetworkPolicy::Deny);
    assert_eq!(ws.network_policy, NetworkPolicy::Deny);
    let ws = workspace(NetworkPolicy::AllowList(vec!["example.com".to_owned()]));
    match &ws.network_policy {
        NetworkPolicy::AllowList(hosts) => assert_eq!(hosts, &["example.com".to_owned()]),
        other => panic!("expected AllowList, got {other:?}"),
    }
}

#[test]
fn workspace_roundtrips_through_json() {
    let ws = workspace(NetworkPolicy::AllowList(vec!["example.com".to_owned()]));
    let json = serde_json::to_string(&ws).expect("must serialize");
    let restored: WorkspaceSpec = serde_json::from_str(&json).expect("must deserialize");
    assert_eq!(ws, restored);
}

/// `deny_unknown_fields`: рабочее место из будущей версии не должно молча
/// терять ограничения при разборе старым читателем — молчаливая потеря
/// `tools_deny` означала бы выполнение команды, которую запретили.
#[test]
fn workspace_rejects_unknown_fields() {
    let bad = serde_json::json!({
        "id": "ws-1", "base_commit": "abc", "network_policy": "deny", "surprise": 1
    });
    assert!(serde_json::from_value::<WorkspaceSpec>(bad).is_err());
}

#[test]
fn workspace_defaulted_collections_may_be_omitted() {
    // repos/tools/env необязательны: минимальное рабочее место — это
    // идентификатор, коммит и сетевая политика.
    let minimal = serde_json::json!({
        "id": "ws-1", "base_commit": "abc", "network_policy": "deny"
    });
    let ws: WorkspaceSpec = serde_json::from_value(minimal).expect("must parse minimal");
    assert!(ws.repos.is_empty());
    assert!(ws.tools_allow.is_empty());
    assert!(ws.tools_deny.is_empty());
}
