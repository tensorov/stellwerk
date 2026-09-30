//! GREEN M4/packet-04: `watch <id>` печатает события, а не отвечает «не реализовано».
//!
//! Три случая обязаны различаться кодом и stdout: задача С событиями, задача БЕЗ
//! событий (код 0 + `no events for <id>`, а не 1 — закон 6) и задача, которой НЕТ
//! (код 1, как у `get`). Без третьего сценарий доказывал бы половину пакета.
//!
//! Запускается НАСТОЯЩИЙ бинарник, а не `cli::run`: тот пишет в реальный stdout и
//! отдаёт код наружу, а тесту нужны и код, и текст раздельно. `STELLWERK_DB` —
//! существующий механизм выбора базы (`cli::db_path`), новая переменная не заводится.
//! Импорт идёт через публичный API крейтов, не `#[path]`: тот собрал бы вторую копию.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use stellwerk_core::contract::{Event, EventRisk, EventSource, RiskClass};
use stellwerk_store::{EventStore, SqliteStore};

/// Код и обе строки раздельно: пакет различает случаи тем, ЧТО ушло в stdout.
struct Ran {
    code: i32,
    out: String,
    err: String,
}

/// Своя база на каждый тест: параллельные прогоны CLI не должны делить файл и ловить
/// `SQLITE_BUSY` (у rusqlite `busy_timeout = 0`).
struct Sandbox {
    db: PathBuf,
    task_file: PathBuf,
}

impl Sandbox {
    fn new(tag: &str) -> Self {
        let stem = format!("sw-watch-{tag}-{}", std::process::id());
        Self {
            db: std::env::temp_dir().join(format!("{stem}.db")),
            task_file: std::env::temp_dir().join(format!("{stem}.toml")),
        }
    }

    fn run(&self, args: &[&str]) -> Ran {
        let out = Command::new(env!("CARGO_BIN_EXE_stellwerk-task"))
            .args(["task"])
            .args(args)
            .env("STELLWERK_DB", &self.db)
            .output()
            .expect("бинарник stellwerk-task обязан запускаться");
        Ran {
            code: out.status.code().expect("процесс обязан завершиться кодом"),
            out: String::from_utf8_lossy(&out.stdout).into_owned(),
            err: String::from_utf8_lossy(&out.stderr).into_owned(),
        }
    }

    /// Создать задачу через `apply` — тем же путём, каким её создаёт человек.
    fn apply(&self, id: &str) {
        let body = format!(
            "[task]\n\
             id = \"{id}\"\n\
             goal = \"Проверить печать событий\"\n\
             workspace_ref = \"ws-watch\"\n\
             model_ref = \"m-default\"\n\
             packet = \"M4/packet-04\"\n\
             limits = {{ max_files = 3, max_diff_lines = 50, timeout_seconds = 60,\n\
             max_retries = 1, context_budget_tokens = 1000, data_class = \"internal\" }}\n"
        );
        fs::write(&self.task_file, body).expect("файл задачи обязан записываться");
        let ran = self.run(&["apply", "--file", &self.task_file.to_string_lossy()]);
        assert_eq!(ran.code, 0, "apply обязан выйти 0; stderr: {}", ran.err);
    }
}

/// `payload["task_id"]` обязателен: `append` выносит его в колонку, и без него
/// событие не попадёт ни в одну выборку по задаче.
fn event(id: &str, task_id: &str, event_type: &str, ts: &str) -> Event {
    let mut payload = BTreeMap::new();
    payload.insert("task_id".to_owned(), task_id.to_owned());
    Event {
        id: id.to_owned(),
        source: EventSource::System,
        event_type: event_type.to_owned(),
        actor: "m-default".to_owned(),
        workspace: Some("ws-watch".to_owned()),
        risk: EventRisk::Classified(RiskClass::R1),
        payload,
        ts: ts.to_owned(),
    }
}

/// Ожидаемая строка `<timestamp> <kind> <краткое описание>`. Отдельного поля
/// «описание» у `Event` нет: краткое описание — это `actor`, взятое дословно.
/// Ожидание собирается из тех же полей, что читает CLI, поэтому ловит расхождение
/// формата, а не повторяет реализацию вслепую.
fn line_of(event: &Event) -> String {
    format!("{} {} {}", event.ts, event.event_type, event.actor)
}

#[tokio::test]
async fn watch_existing_task_with_events_prints_one_line_per_event() {
    let sandbox = Sandbox::new("with-events");
    let id = "t-watch-with-events";
    sandbox.apply(id);

    let store = SqliteStore::open(&sandbox.db).expect("база обязана открываться");
    for (n, (kind, ts)) in [
        ("run.start", "2026-09-29T10:00:00Z"),
        ("run.step", "2026-09-29T10:00:01Z"),
        ("run.end", "2026-09-29T10:00:02Z"),
    ]
    .into_iter()
    .enumerate()
    {
        store
            .append(event(&format!("e-{n}"), id, kind, ts))
            .await
            .expect("событие пишется");
    }
    // Порядок снимается у САМОГО трейта: пакет требует печатать «в порядке, который
    // отдаёт трейт», и единственный честный образец порядка — `list_by_task`.
    let expected: Vec<String> = store
        .list_by_task(id)
        .await
        .expect("выборка обязана читаться")
        .iter()
        .map(line_of)
        .collect();
    assert_eq!(
        expected.len(),
        3,
        "в выборке обязано быть три события, иначе тест доказывает меньше"
    );
    drop(store);

    let ran = sandbox.run(&["watch", id]);
    let lines: Vec<&str> = expected.iter().map(String::as_str).collect();
    assert_eq!(ran.code, 0, "watch обязан выйти 0; stderr: {}", ran.err);
    assert_eq!(
        ran.out.lines().collect::<Vec<_>>(),
        lines,
        "по строке на событие в порядке трейта; получено: {:?}",
        ran.out
    );
}

/// Задача без событий — факт, а не ошибка. Код 1 здесь сделал бы «нет событий»
/// неотличимым от «нет задачи» (закон 6) и выглядел бы поломкой на только что
/// созданной задаче.
#[tokio::test]
async fn watch_existing_task_without_events_succeeds_with_notice() {
    let sandbox = Sandbox::new("no-events");
    let id = "t-watch-no-events";
    sandbox.apply(id);

    let ran = sandbox.run(&["watch", id]);
    assert_eq!(
        ran.code, 0,
        "watch задачи без событий обязан выйти 0, а не 1; stdout: {:?}, stderr: {}",
        ran.out, ran.err
    );
    assert_eq!(
        ran.out,
        format!("no events for {id}\n"),
        "на stdout ровно `no events for <id>`; получено: {:?}",
        ran.out
    );
    assert_eq!(ran.err, "", "случай не ошибка: stderr обязан быть пустым");
}

/// Отсутствующая задача — код 1, как у `get`, и код обязан ОТЛИЧАТЬСЯ от предыдущего
/// случая: иначе два состояния неразличимы.
#[tokio::test]
async fn watch_missing_task_fails_like_get() {
    let sandbox = Sandbox::new("missing");
    let existing_empty = "t-watch-existing-empty";
    sandbox.apply(existing_empty);

    let got = sandbox.run(&["watch", "t-watch-absent"]);
    let get = sandbox.run(&["get", "t-watch-absent"]);
    let empty = sandbox.run(&["watch", existing_empty]);

    assert_eq!(got.code, 1, "watch отсутствующей задачи обязан вернуть 1");
    assert_eq!(
        got.code, get.code,
        "watch и get обязаны одинаково отвечать на отсутствующую задачу"
    );
    assert!(
        got.err.contains("not found"),
        "нет `not found`: {}",
        got.err
    );
    assert_eq!(got.out, "", "нет задачи — пустой stdout: {:?}", got.out);
    // Ключевое различие пакета: «задачи нет» (1) против «задача есть, событий нет» (0).
    assert_ne!(
        got.code, empty.code,
        "отсутствие задачи и отсутствие событий обязаны различаться кодами"
    );
    assert_ne!(
        got.out, empty.out,
        "отсутствие задачи и отсутствие событий обязаны различаться stdout"
    );
}
