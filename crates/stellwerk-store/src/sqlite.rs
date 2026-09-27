//! SQLite-реализация трейтов хранения.
//!
//! ## Что реализовано, а что нет
//!
//! **Реализовано:** открытие базы и применение миграций при первом подключении.
//! Миграции встраиваются в бинарник, поэтому офлайн-установка не зависит от
//! каталога миграций на диске.
//!
//! **Не реализовано (RED-спецификация):** все операции чтения и записи данных.
//! Тела — `todo!()`, поэтому вызов падает с `not implemented`, а не возвращает
//! пустой список. Пустой список был бы хуже: он выглядел бы как «задач нет» и
//! тихо уводил бы federation в состояние с потерей данных.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;
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
        _name: &str,
    ) -> impl std::future::Future<Output = Result<Option<ModuleRecord>, StoreError>> + Send {
        async { todo!("RED: SqliteStore::get — SELECT из modules по имени") }
    }

    fn list(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<ModuleRecord>, StoreError>> + Send {
        async { todo!("RED: SqliteStore::list — SELECT из modules ORDER BY name") }
    }

    fn upsert_lock_entry(
        &self,
        _record: ModuleRecord,
    ) -> impl std::future::Future<Output = Result<(), StoreError>> + Send {
        async {
            todo!("RED: SqliteStore::upsert_lock_entry — INSERT ... ON CONFLICT(name) DO UPDATE")
        }
    }
}
