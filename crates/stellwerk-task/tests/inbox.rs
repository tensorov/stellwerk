//! RED-спецификация M4/packet-07: различение «запрещено политикой» и «никто не ответил»
//! обязано дойти до **статуса задачи в БД**, иначе в расследовании эти два факта
//! неразличимы, а это буквально запрещено В4.
//!
//! Тест проверяет статус через `status_of`, а не только код возврата: функция может
//! вернуть верный статус и никто его не записать, и такой «зелёный» прогон ничего не доказывает.

use std::path::{Path, PathBuf};

use stellwerk_core::contract::{
    DataClass, PolicyLevel, RiskClass, TaskLimits, TaskSpec, TaskStatus,
};
use stellwerk_gateway::permission::ToolRisk;
use stellwerk_store::{SqliteStore, TaskStore};
use stellwerk_task::approval::{inbox_dir, next_status, park, request, StatusEffect};
use stellwerk_task::runner::CallVerdict;

fn spec(id: &str) -> TaskSpec {
    TaskSpec {
        id: id.to_owned(),
        goal: "Реализовать approval".to_owned(),
        workspace_ref: "ws-stellwerk".to_owned(),
        model_ref: "qwen-local".to_owned(),
        packet: "спросить человека перед рискованным вызовом".to_owned(),
        limits: TaskLimits {
            max_files: 3,
            max_diff_lines: 200,
            timeout_seconds: 900,
            max_retries: 2,
            context_budget_tokens: 40_000,
            data_class: DataClass::Internal,
        },
        evidence_policy: "cargo test -p stellwerk-task --test inbox".to_owned(),
    }
}

fn store() -> SqliteStore {
    SqliteStore::open(Path::new(":memory:")).expect("store must open")
}

/// Каталог на время теста: `.stellwerk/inbox` в рабочем каталоге писать нельзя, а
/// параллельные тесты делили бы один путь.
fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("stellwerk-m4-07-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch must be creatable");
    dir
}

async fn running(store: &SqliteStore, id: &str) {
    store.create(spec(id)).await.expect("create must succeed");
    store
        .update_status(id, TaskStatus::Running)
        .await
        .expect("update must succeed");
}

/// Применяет эффект ровно так, как его обязан применять вызывающий: `Keep` не пишет
/// в БД ничего, `Transition` пишет и читает обратно.
async fn apply(
    store: &SqliteStore,
    id: &str,
    verdict: &CallVerdict,
    risk: ToolRisk,
) -> Option<TaskStatus> {
    match next_status(verdict, risk) {
        StatusEffect::Keep => None,
        StatusEffect::Transition(to) => {
            store
                .update_status(id, to)
                .await
                .expect("update must succeed");
            Some(to)
        }
    }
}

#[tokio::test]
async fn ask_without_interaction_suspends_the_task_in_db_and_parks_the_request() {
    let store = store();
    running(&store, "t-ask").await;
    let inbox = scratch("ask");
    let risk = ToolRisk::Classified(RiskClass::R3);

    let req = request("run-ask", "t-ask", "shell", risk);
    let effect = next_status(&CallVerdict::Ask, risk);
    assert!(
        matches!(effect, StatusEffect::Transition(TaskStatus::Suspended)),
        "Ask обязан требовать перехода в Suspended, а не Keep"
    );

    apply(&store, "t-ask", &CallVerdict::Ask, risk).await;
    assert_eq!(
        store.status_of("t-ask").await.expect("read must succeed"),
        Some(TaskStatus::Suspended),
        "в БД обязан быть Suspended, а не Failed"
    );

    park(&req, &inbox).expect("park must succeed");
    let parked = inbox.join("run-ask.json");
    assert!(parked.is_file(), "вопрос обязан лежать в inbox");
}

#[tokio::test]
async fn allow_keeps_the_status_and_writes_nothing_to_the_store() {
    let store = store();
    running(&store, "t-allow").await;
    let inbox = scratch("allow");
    let risk = ToolRisk::Classified(RiskClass::R3);

    let before = store.status_of("t-allow").await.expect("read must succeed");
    let effect = next_status(&CallVerdict::Allow, risk);
    assert!(
        matches!(effect, StatusEffect::Keep),
        "Allow обязан быть Keep, а не переходом"
    );

    assert_eq!(
        apply(&store, "t-allow", &CallVerdict::Allow, risk).await,
        None,
        "Keep не пишет в БД"
    );
    assert_eq!(
        store.status_of("t-allow").await.expect("read must succeed"),
        before,
        "после Keep статус обязан совпасть с исходным, а не смениться на Failed"
    );

    let req = request("run-allow", "t-allow", "shell", risk);
    assert!(
        matches!(next_status(&CallVerdict::Allow, risk), StatusEffect::Keep),
        "Allow остаётся Keep"
    );
    // Keep не значит «спросили и не ответили»: парковать нечего.
    assert!(
        !inbox.join("run-allow.json").exists(),
        "при Allow inbox-файла быть не должно"
    );
    drop(req);
}

#[tokio::test]
async fn parked_json_carries_run_tool_risk_and_expiry() {
    let store = store();
    running(&store, "t-json").await;
    let inbox = scratch("json");
    let risk = ToolRisk::Classified(RiskClass::R4);

    let req = request("run-json", "t-json", "shell", risk);
    assert_eq!(req.window_hours, 4, "R4 — окно 4ч");
    assert_eq!(req.answerable_by, PolicyLevel::L5Owner);
    park(&req, &inbox).expect("park must succeed");

    let raw = std::fs::read_to_string(inbox.join("run-json.json")).expect("must read");
    let v: serde_json::Value = serde_json::from_str(&raw).expect("must parse");
    for field in ["run_id", "tool", "risk", "expires_at", "task_id"] {
        assert!(v.get(field).is_some(), "в JSON нет `{field}`");
    }
    assert_eq!(v["run_id"], serde_json::json!("run-json"));
    assert_eq!(
        v["expires_at"].as_str().map(str::len),
        Some(20),
        "RFC 3339 UTC"
    );

    // R3 — не R4, значит окно сутки, и это обязано быть видно в самом запросе.
    let wide = request(
        "run-json",
        "t-json",
        "shell",
        ToolRisk::Classified(RiskClass::R3),
    );
    assert_eq!(wide.window_hours, 24, "не R4 — окно 24ч");
    let unknown = request("run-json", "t-json", "shell", ToolRisk::Unclassified);
    assert_eq!(unknown.window_hours, 24);
}

#[tokio::test]
async fn r5_at_owner_is_failed_and_parks_nothing() {
    let store = store();
    running(&store, "t-r5").await;
    let inbox = scratch("r5");
    let risk = ToolRisk::Classified(RiskClass::R5);

    let effect = next_status(&CallVerdict::Ask, risk);
    assert!(
        matches!(effect, StatusEffect::Transition(TaskStatus::Failed)),
        "R5 необратимо: Suspended ждал бы ответа, которого не будет"
    );

    apply(&store, "t-r5", &CallVerdict::Ask, risk).await;
    assert_eq!(
        store.status_of("t-r5").await.expect("read must succeed"),
        Some(TaskStatus::Failed),
        "R5 обязан дать Failed, а не Suspended"
    );
    assert!(
        !inbox.join("t-r5.json").exists(),
        "вопрос, который нельзя решить, парковать нельзя"
    );
}

#[tokio::test]
async fn unclassified_is_failed_and_parks_nothing() {
    let store = store();
    running(&store, "t-unk").await;
    let inbox = scratch("unk");

    let effect = next_status(&CallVerdict::Ask, ToolRisk::Unclassified);
    assert!(
        matches!(effect, StatusEffect::Transition(TaskStatus::Failed)),
        "Unclassified: вопрос задать некому, Suspended ждал бы ответа, которого не будет"
    );

    apply(&store, "t-unk", &CallVerdict::Ask, ToolRisk::Unclassified).await;
    assert_eq!(
        store.status_of("t-unk").await.expect("read must succeed"),
        Some(TaskStatus::Failed)
    );
    assert!(!inbox.join("t-unk.json").exists());
}

#[tokio::test]
async fn suspended_verdict_also_suspends() {
    let risk = ToolRisk::Classified(RiskClass::R3);
    assert!(matches!(
        next_status(&CallVerdict::Suspended, risk),
        StatusEffect::Transition(TaskStatus::Suspended)
    ));
}

#[tokio::test]
async fn deny_is_failed() {
    let risk = ToolRisk::Classified(RiskClass::R3);
    let deny = CallVerdict::Deny {
        reason: "risk_denied: класс R5 запрещён".to_owned(),
    };
    assert!(matches!(
        next_status(&deny, risk),
        StatusEffect::Transition(TaskStatus::Failed)
    ));
}

/// Отсутствие значения и его пустое значение — разные случаи: пустое обязано быть
/// ошибкой, иначе опечатка молча пишет вопросы в рабочий каталог. Env-переменную
/// читает точка входа; сюда попадает уже разрешённое значение, заданное параметром,
/// поэтому глобальное окружение тест не трогает.
#[test]
fn empty_inbox_env_is_an_error_not_a_silent_default() {
    let outcome = inbox_dir(Some(""));
    assert!(
        outcome.is_err(),
        "пустое значение обязано давать ошибку, а не умолчание"
    );
}

#[test]
fn missing_inbox_env_falls_back_to_workspace_path() {
    let outcome = inbox_dir(None);
    assert_eq!(
        outcome.expect("отсутствие значения обязано давать умолчание"),
        PathBuf::from(".stellwerk/inbox"),
        "умолчание — путь внутри рабочего каталога"
    );
}
