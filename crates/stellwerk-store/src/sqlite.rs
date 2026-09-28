//! SQLite-реализация трейтов хранения.
//!
//! ## Что реализовано, а что нет
//!
//! **Реализовано:** открытие базы и применение миграций при первом подключении.
//! Миграции встраиваются в бинарник, поэтому офлайн-установка не зависит от
//! каталога миграций на диске.
//!
//! **Состояние:** все операции чтения и записи **реализованы** пакетами M1
//! (сессия #2, 2026-09-27). Раньше тела были `todo!()`, и вызов падал с
//! `not implemented`, а не возвращал пустой список: пустой список выглядел бы
//! как «задач нет» и тихо уводил бы федерацию в состояние с потерей данных.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension};
use stellwerk_core::contract::{Event, TaskSpec, TaskStatus};

use crate::migrations::MIGRATIONS;
use crate::traits::{EventStore, ModuleRecord, ModuleStore, StoreError, TaskStore};

/// Serialize a TaskStatus to its PascalCase string (matching serde rename_all).
fn status_to_str(s: TaskStatus) -> &'static str {
    match s {
        TaskStatus::Draft => "Draft",
        TaskStatus::Planned => "Planned",
        TaskStatus::Reviewed => "Reviewed",
        TaskStatus::Delegated => "Delegated",
        TaskStatus::Running => "Running",
        TaskStatus::Suspended => "Suspended",
        TaskStatus::EvidenceSubmitted => "EvidenceSubmitted",
        TaskStatus::Verified => "Verified",
        TaskStatus::Validated => "Validated",
        TaskStatus::Done => "Done",
        TaskStatus::Failed => "Failed",
        TaskStatus::Blocked => "Blocked",
    }
}

/// Обратная сторона `status_to_str`: разбирает PascalCase-строку базы в статус.
/// Неизвестная строка — ошибка, а не `Draft`: значение по умолчанию неотличимо от
/// сознательного решения, а решения здесь не было.
fn str_to_status(s: &str) -> Result<TaskStatus, StoreError> {
    match s {
        "Draft" => Ok(TaskStatus::Draft),
        "Planned" => Ok(TaskStatus::Planned),
        "Reviewed" => Ok(TaskStatus::Reviewed),
        "Delegated" => Ok(TaskStatus::Delegated),
        "Running" => Ok(TaskStatus::Running),
        "Suspended" => Ok(TaskStatus::Suspended),
        "EvidenceSubmitted" => Ok(TaskStatus::EvidenceSubmitted),
        "Verified" => Ok(TaskStatus::Verified),
        "Validated" => Ok(TaskStatus::Validated),
        "Done" => Ok(TaskStatus::Done),
        "Failed" => Ok(TaskStatus::Failed),
        "Blocked" => Ok(TaskStatus::Blocked),
        // `Serialization` не годится: он принимает `serde_json::Error`, а не текст.
        // `NotFound`/`Conflict` врут про задачу, которая как раз существует.
        _ => Err(StoreError::Sqlite(format!(
            "unknown task status in db: {s}"
        ))),
    }
}

/// Хранилище на SQLite.
///
/// `Mutex<Connection>`, а не пул: rusqlite-соединение не является `Sync`, а трейты
/// требуют `Send + Sync`. Одна блокировка на всё соединение — сознательная
/// уступка простоте: федерация пишет мало и пишет последовательно. Переход на
/// пул — отдельный packet, когда появится измеримая потребность.
#[derive(Debug)]
pub struct SqliteStore {
    conn: Mutex<Connection>,
}

impl SqliteStore {
    /// Открывает базу и применяет миграции, ещё не применённые.
    ///
    /// `path` может быть `:memory:` — полезно для тестов и для dry-run.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let conn = Connection::open(path).map_err(|e| StoreError::Sqlite(e.to_string()))?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.migrate()?;
        Ok(store)
    }

    /// Применяет миграции, отсутствующие в `schema_migrations`.
    fn migrate(&self) -> Result<(), StoreError> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                 version    TEXT PRIMARY KEY,
                 applied_at TEXT NOT NULL
             );",
        )
        .map_err(|e| StoreError::Migration(e.to_string()))?;

        for (version, sql) in MIGRATIONS {
            let applied: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM schema_migrations WHERE version = ?1",
                    [version],
                    |row| row.get(0),
                )
                .map_err(|e| StoreError::Migration(e.to_string()))?;
            if applied > 0 {
                continue;
            }
            conn.execute_batch(sql)
                .map_err(|e| StoreError::Migration(format!("{version}: {e}")))?;
            conn.execute(
                "INSERT INTO schema_migrations (version, applied_at)
                 VALUES (?1, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))",
                [version],
            )
            .map_err(|e| StoreError::Migration(e.to_string()))?;
        }
        Ok(())
    }

    /// Вспомогательная: сериализует TaskSpec в JSON для хранения.
    fn spec_to_json(spec: &TaskSpec) -> Result<String, StoreError> {
        serde_json::to_string(spec).map_err(StoreError::Serialization)
    }

    /// Вспомогательная: десериализует JSON в TaskSpec.
    fn json_to_spec(json: &str) -> Result<TaskSpec, StoreError> {
        serde_json::from_str(json).map_err(StoreError::Serialization)
    }
}

impl TaskStore for SqliteStore {
    fn create(
        &self,
        spec: TaskSpec,
    ) -> impl std::future::Future<Output = Result<TaskSpec, StoreError>> + Send {
        async {
            let json = Self::spec_to_json(&spec)?;
            let status = TaskStatus::Draft;

            let conn = self.conn.lock().expect("store mutex poisoned");
            let result = conn.execute(
                "INSERT INTO tasks (id, status, workspace_id, data, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))",
                rusqlite::params![spec.id, status_to_str(status), spec.workspace_ref, json],
            );

            match result {
                Ok(_) => Ok(spec),
                Err(rusqlite::Error::SqliteFailure(err, _))
                    if err.code == rusqlite::ErrorCode::ConstraintViolation =>
                {
                    Err(StoreError::Conflict(format!(
                        "task already exists: {}",
                        spec.id
                    )))
                }
                Err(e) => Err(StoreError::Sqlite(e.to_string())),
            }
        }
    }

    fn get(
        &self,
        id: &str,
    ) -> impl std::future::Future<Output = Result<Option<TaskSpec>, StoreError>> + Send {
        async move {
            let conn = self.conn.lock().expect("store mutex poisoned");
            let row = conn.query_row(
                "SELECT data FROM tasks WHERE id = ?1",
                rusqlite::params![id],
                |row| row.get::<_, String>(0),
            );
            match row {
                Ok(json) => Ok(Some(Self::json_to_spec(&json)?)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(StoreError::Sqlite(e.to_string())),
            }
        }
    }

    fn update_status(
        &self,
        id: &str,
        status: TaskStatus,
    ) -> impl std::future::Future<Output = Result<TaskSpec, StoreError>> + Send {
        async move {
            let conn = self.conn.lock().expect("store mutex poisoned");
            let changes = conn.execute(
                "UPDATE tasks SET status = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?1",
                rusqlite::params![id, status_to_str(status)],
            );

            match changes {
                Ok(0) => Err(StoreError::NotFound(format!("task not found: {id}"))),
                Ok(_) => {
                    let row = conn.query_row(
                        "SELECT data FROM tasks WHERE id = ?1",
                        rusqlite::params![id],
                        |row| row.get::<_, String>(0),
                    );
                    match row {
                        Ok(json) => Ok(Self::json_to_spec(&json)?),
                        Err(e) => Err(StoreError::Sqlite(e.to_string())),
                    }
                }
                Err(e) => Err(StoreError::Sqlite(e.to_string())),
            }
        }
    }

    fn status_of(
        &self,
        id: &str,
    ) -> impl std::future::Future<Output = Result<Option<TaskStatus>, StoreError>> + Send {
        async move {
            let conn = self.conn.lock().expect("store mutex poisoned");
            let row = conn
                .query_row(
                    "SELECT status FROM tasks WHERE id = ?1",
                    rusqlite::params![id],
                    |row| row.get::<_, String>(0),
                )
                .optional();

            match row {
                // Строка есть, но значение нечитаемо — ошибка разбора, а не `None`:
                // «нет статуса» и «статус неизвестен» разные вещи.
                Ok(Some(raw)) => Ok(Some(str_to_status(&raw)?)),
                Ok(None) => Ok(None),
                Err(e) => Err(StoreError::Sqlite(e.to_string())),
            }
        }
    }

    fn compare_and_set_status(
        &self,
        id: &str,
        expected: TaskStatus,
        next: TaskStatus,
    ) -> impl std::future::Future<Output = Result<bool, StoreError>> + Send {
        async move {
            let conn = self.conn.lock().expect("store mutex poisoned");
            // Предикат живёт ВНУТРИ UPDATE: «прочитал, потом записал» — это два
            // разных вызова, между которыми статус успевает сменить другой
            // исполнитель, и его запись стёрлась бы.
            let changes = conn.execute(
                "UPDATE tasks SET status = ?3, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?1 AND status = ?2",
                rusqlite::params![id, status_to_str(expected), status_to_str(next)],
            );

            // В SQLite ноль обновлённых строк — не ошибка, поэтому счётчик
            // проверяется здесь. `Ok(false)` означает и «статус не совпал», и
            // «задачи нет»; вызывающий обязан считать оба отказом.
            match changes {
                Ok(0) => Ok(false),
                Ok(1) => Ok(true),
                Ok(n) => Err(StoreError::Sqlite(format!(
                    "compare_and_set_status touched {n} rows for a single id: {id}"
                ))),
                Err(e) => Err(StoreError::Sqlite(e.to_string())),
            }
        }
    }

    fn list_by_status(
        &self,
        status: TaskStatus,
    ) -> impl std::future::Future<Output = Result<Vec<TaskSpec>, StoreError>> + Send {
        async move {
            let conn = self.conn.lock().expect("store mutex poisoned");
            let mut stmt = conn
                .prepare("SELECT data FROM tasks WHERE status = ?1 ORDER BY id")
                .map_err(|e| StoreError::Sqlite(e.to_string()))?;
            let rows = stmt
                .query_map(rusqlite::params![status_to_str(status)], |row| {
                    let json: String = row.get(0)?;
                    Ok(json)
                })
                .map_err(|e| StoreError::Sqlite(e.to_string()))?;
            let mut specs = Vec::new();
            for row_result in rows {
                let json: String = row_result.map_err(|e| StoreError::Sqlite(e.to_string()))?;
                specs.push(Self::json_to_spec(&json)?);
            }
            Ok(specs)
        }
    }

    fn list_by_workspace(
        &self,
        workspace_id: &str,
    ) -> impl std::future::Future<Output = Result<Vec<TaskSpec>, StoreError>> + Send {
        async move {
            let conn = self.conn.lock().expect("store mutex poisoned");
            let mut stmt = conn
                .prepare("SELECT data FROM tasks WHERE workspace_id = ?1 ORDER BY id")
                .map_err(|e| StoreError::Sqlite(e.to_string()))?;
            let rows = stmt
                .query_map(rusqlite::params![workspace_id], |row| {
                    let json: String = row.get(0)?;
                    Ok(json)
                })
                .map_err(|e| StoreError::Sqlite(e.to_string()))?;
            let mut specs = Vec::new();
            for row_result in rows {
                let json: String = row_result.map_err(|e| StoreError::Sqlite(e.to_string()))?;
                specs.push(Self::json_to_spec(&json)?);
            }
            Ok(specs)
        }
    }
}

impl EventStore for SqliteStore {
    fn append(
        &self,
        event: Event,
    ) -> impl std::future::Future<Output = Result<(), StoreError>> + Send {
        async move {
            let task_id = event.payload.get("task_id").cloned();
            let data_json = serde_json::to_string(&event).map_err(StoreError::Serialization)?;

            let conn = self.conn.lock().expect("store mutex poisoned");
            let result = conn.execute(
                "INSERT INTO events (id, task_id, ts, created_at, data)
                 VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), ?4)",
                rusqlite::params![event.id, task_id.as_deref(), event.ts, data_json,],
            );

            match result {
                Ok(_) => Ok(()),
                Err(rusqlite::Error::SqliteFailure(err, _))
                    if err.code == rusqlite::ErrorCode::ConstraintViolation =>
                {
                    Err(StoreError::Conflict(format!(
                        "event already exists: {}",
                        event.id
                    )))
                }
                Err(e) => Err(StoreError::Sqlite(e.to_string())),
            }
        }
    }

    fn list_since(
        &self,
        ts: &str,
    ) -> impl std::future::Future<Output = Result<Vec<Event>, StoreError>> + Send {
        async move {
            let conn = self.conn.lock().expect("store mutex poisoned");
            let mut stmt = conn
                .prepare("SELECT data FROM events WHERE ts >= ?1 ORDER BY ts, id")
                .map_err(|e| StoreError::Sqlite(e.to_string()))?;
            let rows = stmt
                .query_map(rusqlite::params![ts], |row| {
                    let data_json: String = row.get(0)?;
                    serde_json::from_str(&data_json)
                        .map_err(|_e: serde_json::Error| rusqlite::Error::QueryReturnedNoRows)
                })
                .map_err(|e| StoreError::Sqlite(e.to_string()))?;

            let mut events = Vec::new();
            for row_result in rows {
                events.push(row_result.map_err(|e| StoreError::Sqlite(e.to_string()))?);
            }
            Ok(events)
        }
    }

    fn list_by_task(
        &self,
        task_id: &str,
    ) -> impl std::future::Future<Output = Result<Vec<Event>, StoreError>> + Send {
        async move {
            let conn = self.conn.lock().expect("store mutex poisoned");
            let mut stmt = conn
                .prepare("SELECT data FROM events WHERE task_id = ?1 ORDER BY ts, id")
                .map_err(|e| StoreError::Sqlite(e.to_string()))?;
            let rows = stmt
                .query_map(rusqlite::params![task_id], |row| {
                    let data_json: String = row.get(0)?;
                    serde_json::from_str(&data_json)
                        .map_err(|_e: serde_json::Error| rusqlite::Error::QueryReturnedNoRows)
                })
                .map_err(|e| StoreError::Sqlite(e.to_string()))?;

            let mut events = Vec::new();
            for row_result in rows {
                events.push(row_result.map_err(|e| StoreError::Sqlite(e.to_string()))?);
            }
            Ok(events)
        }
    }
}

impl ModuleStore for SqliteStore {
    fn get(
        &self,
        name: &str,
    ) -> impl std::future::Future<Output = Result<Option<ModuleRecord>, StoreError>> + Send {
        async move {
            let conn = self.conn.lock().expect("store mutex poisoned");
            let row = conn.query_row(
                "SELECT data FROM modules WHERE name = ?1",
                rusqlite::params![name],
                |row| row.get::<_, String>(0),
            );
            match row {
                Ok(json) => Ok(Some(serde_json::from_str(&json)?)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(StoreError::Sqlite(e.to_string())),
            }
        }
    }

    fn list(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<ModuleRecord>, StoreError>> + Send {
        async move {
            let conn = self.conn.lock().expect("store mutex poisoned");
            let mut stmt = conn
                .prepare("SELECT data FROM modules ORDER BY name")
                .map_err(|e| StoreError::Sqlite(e.to_string()))?;
            let rows = stmt
                .query_map(rusqlite::params![], |row| {
                    let json: String = row.get(0)?;
                    Ok(json)
                })
                .map_err(|e| StoreError::Sqlite(e.to_string()))?;
            let mut records = Vec::new();
            for row_result in rows {
                let json: String = row_result.map_err(|e| StoreError::Sqlite(e.to_string()))?;
                records.push(serde_json::from_str(&json)?);
            }
            Ok(records)
        }
    }

    fn upsert_lock_entry(
        &self,
        record: ModuleRecord,
    ) -> impl std::future::Future<Output = Result<(), StoreError>> + Send {
        async move {
            let data_json = serde_json::to_string(&record)?;
            let risk_str = record
                .risk
                .as_ref()
                .map(|r| serde_json::to_string(r).unwrap());

            let name = record.name.clone();
            let conn = self.conn.lock().expect("store mutex poisoned");

            // Check if a row with this name already exists.
            let existing: Option<String> = conn
                .query_row(
                    "SELECT id FROM modules WHERE name = ?1",
                    rusqlite::params![&name],
                    |row| row.get(0),
                )
                .ok();

            let id = existing.unwrap_or_else(|| name.clone());

            // On CONFLICT(id): update name/risk/data/updated_at, keep created_at.
            let result = conn.execute(
                "INSERT INTO modules (id, name, risk, data, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, \
                         strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), \
                         strftime('%Y-%m-%dT%H:%M:%SZ', 'now')) \
                 ON CONFLICT(id) DO UPDATE SET \
                     name=excluded.name, \
                     risk=excluded.risk, \
                     data=excluded.data, \
                     updated_at=excluded.updated_at, \
                     created_at=modules.created_at",
                rusqlite::params![id, &name, risk_str.as_deref(), data_json],
            );

            match result {
                Ok(_) => Ok(()),
                Err(rusqlite::Error::SqliteFailure(err, _))
                    if err.code == rusqlite::ErrorCode::ConstraintViolation =>
                {
                    Err(StoreError::Conflict(format!("module conflict: {name}")))
                }
                Err(e) => Err(StoreError::Sqlite(e.to_string())),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    /// Битая строка обязана дать ошибку, а не `Ok(Draft)`. Проверка здесь, а не в
    /// интеграционном тесте: мусор в колонку пишется только прямым SQL.
    #[test]
    fn str_to_status_rejects_unknown_string_instead_of_defaulting() {
        let parsed = super::str_to_status("НеСтатус");
        assert!(
            parsed.is_err(),
            "неизвестная строка обязана дать Err, а не Ok(Draft): получено {parsed:?}"
        );
    }
}
