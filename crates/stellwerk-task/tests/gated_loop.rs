//! N2-00b: решение по вызову — ДО эффекта; первый не-`Allow` останавливает проход. Негативный тест: вызов запрошен, эффекта нет, хвост не запрашивался.

use std::path::{Path, PathBuf};
use std::time::Duration;

use stellwerk_core::contract::{DataClass, ModelBinding, PolicyLevel, RetentionClass, RiskClass};
use stellwerk_gateway::permission::{PermissionError, ToolRisk};
use stellwerk_store::{EventStore, SqliteStore};
use stellwerk_task::m5::sandbox_bwrap::Sandbox;
use stellwerk_task::m5::FixtureEngine;
use stellwerk_task::orchestrator::SandboxWorker;
use stellwerk_task::runner::{
    run_gated, Call, CallAttempt, CallVerdict, GatedPass, RiskRegistry, Worker,
};

const WINDOW: Duration = Duration::from_secs(60);
const BUDGET: Duration = Duration::from_secs(30);

/// Доверенный реестр: класс берётся отсюда, а не из самодекларации `call.risk`.
struct TestRegistry;

impl RiskRegistry for TestRegistry {
    fn risk_of(&self, tool: &str, module: &str) -> Result<ToolRisk, PermissionError> {
        let missing = || PermissionError::TableIncomplete {
            risk: RiskClass::R0,
        };
        if module != "fs" {
            return Err(missing());
        }
        match tool {
            "bash" => Ok(ToolRisk::Classified(RiskClass::R0)),
            "r2" => Ok(ToolRisk::Classified(RiskClass::R2)),
            "r5" => Ok(ToolRisk::Classified(RiskClass::R5)),
            _ => Err(missing()),
        }
    }
}

/// Оборванный канал: реестр отвечает ошибкой на каждый запрос — отказ, не `Allow`.
struct BrokenRegistry;

impl RiskRegistry for BrokenRegistry {
    fn risk_of(&self, _tool: &str, _module: &str) -> Result<ToolRisk, PermissionError> {
        Err(PermissionError::TableIncomplete {
            risk: RiskClass::R5,
        })
    }
}

fn fixture_root(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("sw-n2-00b-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    root
}

fn sandbox(root: &Path) -> Sandbox {
    let work = root.join("work");
    let tmp = root.join("tmp");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::create_dir_all(&tmp).unwrap();
    let read_only = ["/usr", "/bin", "/lib", "/lib64", "/etc"]
        .iter()
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .collect();
    Sandbox {
        work,
        tmp,
        read_only,
        window: WINDOW,
    }
}

fn marker(root: &Path) -> PathBuf {
    root.join("work").join("STARTED")
}

fn binding() -> ModelBinding {
    ModelBinding {
        id: "n2-00b".into(),
        provider: "local".into(),
        model_id: "fixture".into(),
        data_class: DataClass::Public,
        retention: RetentionClass::FirstParty,
        context_window: 8000,
        max_output: 2000,
        budget_tokens_per_task: 20000,
    }
}

/// `risk: Unclassified` намеренно: эффективный риск обязан прийти из реестра.
fn call(task: &str, tool: &str, interactive: bool) -> CallAttempt {
    CallAttempt {
        task_id: task.into(),
        call: Call {
            tool: tool.into(),
            module: "fs".into(),
            risk: ToolRisk::Unclassified,
            data: DataClass::Public,
            model: binding(),
            interactive,
        },
        caller: PolicyLevel::L5Owner,
        workspace_ref: "ws-n2-00b".into(),
    }
}

fn worker(root: &Path, registry: impl RiskRegistry + 'static) -> SandboxWorker<FixtureEngine> {
    let touch = ["/usr/bin/touch", "/bin/touch"]
        .iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
        .unwrap();
    SandboxWorker::new(
        FixtureEngine::new("/bin/true"),
        sandbox(root),
        touch,
        vec!["STARTED".into()],
        BUDGET,
        vec![],
    )
    .with_registry(registry)
}

fn harness(tag: &str) -> (PathBuf, SandboxWorker<FixtureEngine>, SqliteStore) {
    let root = fixture_root(tag);
    let w = worker(&root, TestRegistry);
    (root, w, SqliteStore::open(Path::new(":memory:")).unwrap())
}

fn blocked(pass: &GatedPass) -> Vec<CallVerdict> {
    match pass {
        GatedPass::Blocked(verdicts) => verdicts.clone(),
        other => panic!("ожидался Blocked, получено {other:?}"),
    }
}

async fn status(s: &SqliteStore, task: &str) -> Option<String> {
    s.list_by_task(task)
        .await
        .unwrap()
        .first()
        .and_then(|e| e.payload.get("status").cloned())
}

/// Все статусы задачи в порядке записи (`ORDER BY ts, id`). Нужен там, где по одной
/// задаче пишется больше одного события (первый проход + возобновление).
async fn statuses(s: &SqliteStore, task: &str) -> Vec<String> {
    s.list_by_task(task)
        .await
        .unwrap()
        .into_iter()
        .filter_map(|e| e.payload.get("status").cloned())
        .collect()
}

#[tokio::test]
async fn ask_stops_the_pass_and_the_tail_is_never_requested() {
    let (_, w, s) = harness("ask");
    let requested = vec![
        call("a1", "bash", false),
        call("a2", "r2", true),
        call("a3", "r5", false),
        call("a4", "bash", false),
    ];
    let pass = run_gated(&requested, &w, &s).await.unwrap();
    assert_eq!(
        blocked(&pass),
        vec![CallVerdict::Allow, CallVerdict::Ask],
        "обработан префикс до Ask включительно"
    );
    assert_eq!(w.spawned(), 1, "исполнен только первый вызов");
    assert_eq!(status(&s, "a2").await.as_deref(), Some("Asked"));
    assert!(
        s.list_by_task("a3").await.unwrap().is_empty(),
        "a3 не одобрялся"
    );
    assert!(
        s.list_by_task("a4").await.unwrap().is_empty(),
        "a4 не одобрялся"
    );
}

/// Ответ на `Ask` не воскрешает старую пачку. `run_gated` держит набор вызовов только
/// в аргументе: у него нет `self` и нет статики с индексом пачки, поэтому продолжение
/// идёт НОВЫМ проходом с НОВЫМ набором, а `Allow_B` из исходной пачки сам по себе не
/// исполняется.
///
/// Характеризующий (lock) тест: поведение верно by construction, RED без внесения
/// самого дефекта недостижим. Тест фиксирует отсутствие скрытого состояния между
/// вызовами — против реализации со скрытым курсором/хвостом он падал бы: хвост `p3`
/// исполнился бы, у `p3` появился бы чек. Возобновление берёт ТОТ ЖЕ `task_id`, что
/// был остановлен (`p2`): состояние, ключёванное по задаче, обязано быть поймано, а
/// не проскочить мимо нового id. Эффект проверяется по `spawned()` (счётчик exec) и
/// чеку `Succeeded`, а не по общему `STARTED`: его создаёт уже первый проход.
#[tokio::test]
async fn an_answered_ask_does_not_resume_the_old_tail() {
    let (_, w, s) = harness("resume");
    let first = [
        call("p1", "bash", false), // Allow — исполняется
        call("p2", "r2", true),    // Ask — проход остановлен
        call("p3", "bash", false), // Allow_B — хвост, не должен исполняться
    ];
    let pass = run_gated(&first, &w, &s).await.unwrap();
    assert_eq!(
        blocked(&pass),
        vec![CallVerdict::Allow, CallVerdict::Ask],
        "исходная пачка: Allow, затем Ask"
    );
    assert_eq!(w.spawned(), 1, "исполнен только первый вызов");
    assert_eq!(status(&s, "p2").await.as_deref(), Some("Asked"));
    assert!(
        s.list_by_task("p3").await.unwrap().is_empty(),
        "хвост p3 не запрашивался"
    );

    // Человек ответил на `Ask` задачи p2. Возобновление — НОВЫЙ проход с НОВЫМ
    // набором и с тем же `task_id`, а не продолжение старого вектора с индекса 2.
    let resumed = [call("p2", "bash", false)];
    match run_gated(&resumed, &w, &s).await.unwrap() {
        GatedPass::Completed(v) => assert_eq!(v, vec![CallVerdict::Allow]),
        other => panic!("ожидался Completed, получено {other:?}"),
    }
    assert_eq!(
        w.spawned(),
        2,
        "исполнено ровно то, что передано в новый проход"
    );
    let p2 = statuses(&s, "p2").await;
    assert!(
        p2.contains(&"Asked".to_string()),
        "чек первого прохода сохранён, а не затёрт: {p2:?}"
    );
    assert!(
        p2.contains(&"Succeeded".to_string()),
        "возобновлённый вызов дал эффект: {p2:?}"
    );
    assert!(
        s.list_by_task("p3").await.unwrap().is_empty(),
        "Allow_B из исходной пачки не воскрешён ответом на Ask"
    );

    // Положительный контроль: явно переданный p3 ИСПОЛНЯЕТСЯ. Значит прежние
    // «p3 пуст» — следствие отсутствия воскрешения, а не неработоспособности p3.
    let explicit = [call("p3", "bash", false)];
    match run_gated(&explicit, &w, &s).await.unwrap() {
        GatedPass::Completed(v) => assert_eq!(v, vec![CallVerdict::Allow]),
        other => panic!("ожидался Completed, получено {other:?}"),
    }
    assert_eq!(w.spawned(), 3, "явно переданный p3 исполнен");
    assert_eq!(status(&s, "p3").await.as_deref(), Some("Succeeded"));
}

#[tokio::test]
async fn deny_stops_the_pass_and_the_tail_is_never_requested() {
    let (_, w, s) = harness("deny");
    let requested = vec![
        call("b1", "bash", false),
        call("b2", "r5", false),
        call("b3", "bash", false),
    ];
    let pass = run_gated(&requested, &w, &s).await.unwrap();
    let verdicts = blocked(&pass);
    assert_eq!(verdicts.len(), 2, "обработан префикс до Deny включительно");
    assert_eq!(verdicts[0], CallVerdict::Allow);
    match &verdicts[1] {
        CallVerdict::Deny { reason } => assert!(
            reason.contains("risk_denied"),
            "R5 — Deny из таблицы, не Suspend: {reason}"
        ),
        other => panic!("ожидался Deny, получено {other:?}"),
    }
    assert_eq!(w.spawned(), 1, "исполнен только первый вызов");
    assert_eq!(status(&s, "b2").await.as_deref(), Some("Denied"));
    assert!(
        s.list_by_task("b3").await.unwrap().is_empty(),
        "b3 не запрашивался"
    );
}

#[tokio::test]
async fn suspended_stops_the_pass_and_the_tail_is_never_requested() {
    let (_, w, s) = harness("susp");
    let requested = [
        call("e1", "bash", false),
        call("e2", "r2", false),
        call("e3", "bash", false),
    ];
    let pass = run_gated(&requested, &w, &s).await.unwrap();
    assert_eq!(
        blocked(&pass),
        vec![CallVerdict::Allow, CallVerdict::Suspended]
    );
    assert_eq!(w.spawned(), 1);
    assert_eq!(status(&s, "e2").await.as_deref(), Some("Suspended"));
    assert!(s.list_by_task("e3").await.unwrap().is_empty());
}

#[tokio::test]
async fn an_all_allow_pass_completes() {
    let (root, w, s) = harness("ok");
    let requested = vec![call("c1", "bash", false), call("c2", "bash", false)];
    let pass = run_gated(&requested, &w, &s).await.unwrap();
    match pass {
        GatedPass::Completed(v) => {
            assert_eq!(v, vec![CallVerdict::Allow, CallVerdict::Allow])
        }
        other => panic!("ожидался Completed, получено {other:?}"),
    }
    assert_eq!(w.spawned(), 2);
    assert!(marker(&root).exists(), "разрешённые вызовы дают эффект");
}

#[tokio::test]
async fn a_broken_authorization_channel_refuses_and_produces_no_effect() {
    let root = fixture_root("broken");
    let w = worker(&root, BrokenRegistry);
    let s = SqliteStore::open(Path::new(":memory:")).unwrap();
    let pass = run_gated(&[call("d1", "bash", false)], &w, &s)
        .await
        .unwrap();
    match &blocked(&pass)[..] {
        [CallVerdict::Deny { reason }] => assert!(
            reason.contains("registry_refused"),
            "канал обязан назваться: {reason}"
        ),
        other => panic!("ожидался один Deny, получено {other:?}"),
    }
    assert_eq!(w.spawned(), 0, "воркер не вызван: отказ вынесен до exec");
    assert!(!marker(&root).exists(), "при оборванном канале эффекта нет");
}
