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
use rusqlite::OptionalExtension;
use stellwerk_core::contract::{Event, TaskSpec, TaskStatus};

use crate::migrations::MIGRATIONS;
use crate::traits::{EventStore, ModuleRecord, ModuleStore, StoreError, TaskStore};

/// Текущее время в формате RFC 3339 UTC.
fn now_rfc3339() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_secs();
    // Convert unix epoch to a simple RFC 3339 string.
    // We use a naive approach: compute date/time from unix epoch.
    // For simplicity, use strftime via SQLite — but we need the value
    // in Rust. Use a helper: seconds since epoch -> YYYY-MM-DDTHH:MM:SSZ.
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Compute year/month/day from days since 1970-01-01
    let (y, m, d) = days_to_ymd(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y, m, d, hours, minutes, seconds
    )
}

/// Convert days since 1970-01-01 to (year, month, day).
fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    // Days from 1970-01-01
    let n = days as i64;
    // Convert to Julian day number
    let jdn = n + 2440588;
    // Convert JDN to calendar date (Fliegel-Van Flanden algorithm)
    let f = jdn + 1401 + (((4 * jdn + 274277) / 146097) * 3) / 4 - 38;
    let e = 4 * f + 3;
    let g = (e % 1461) / 4;
    let h = 5 * g + 2;
    let day = (h % 153) / 5 + 1;
    let month = ((h / 153 + 2) % 12) + 1;
    let year = (e / 1461) + 1900 + ((243 - month) / 12);
    (year as u64, month as u64, day as u64)
}

/// Струнное представление TaskStatus (PascalCase, как в схеме).
fn status_str(s: TaskStatus) -> &'static str {
    match s {
        TaskStatus::Draft => "Draft",
        TaskStatus::Planned => "Planned",
        TaskStatus::Reviewed => "Reviewed",
        TaskStatus::Delegated => "Delegated",
        TaskStatus::Running => "Running",
        TaskStatus::Suspended => "Suspended",
        TaskStatus::EvidenceSubmitted => "EvidenceSubmitted",
        TaskStatus::Done => "Done",
        TaskStatus::Failed => "Failed",
        TaskStatus::Verified => "Verified",
        TaskStatus::Validated => "Validated",
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
}

impl TaskStore for SqliteStore {
    fn create(
        &self,
        spec: TaskSpec,
    ) -> impl std::future::Future<Output = Result<TaskSpec, StoreError>> + Send {
        async move {
            let data = serde_json::to_string(&spec).map_err(StoreError::Serialization)?;
            let now = now_rfc3339();
            let workspace_id = spec.workspace_ref.clone();
            let conn = self.conn.lock().expect("store mutex poisoned");
            conn.execute(
                "INSERT INTO tasks (id, status, workspace_id, data, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                [
                    &spec.id,
                    status_str(TaskStatus::Draft),
                    &workspace_id,
                    &data,
                    &now,
                    &now,
                ],
            )
            .map_err(|e| {
                if e.to_string().contains("UNIQUE") {
                    StoreError::Conflict(format!("task {} already exists", spec.id))
                } else {
                    StoreError::Sqlite(e.to_string())
                }
            })?;
            Ok(spec)
        }
    }

    fn get(
        &self,
        id: &str,
    ) -> impl std::future::Future<Output = Result<Option<TaskSpec>, StoreError>> + Send {
        async move {
            let conn = self.conn.lock().expect("store mutex poisoned");
            let data: Option<String> = conn
                .query_row("SELECT data FROM tasks WHERE id = ?1", [id], |row| {
                    row.get(0)
                })
                .optional()
                .map_err(|e| StoreError::Sqlite(e.to_string()))?;
            match data {
                Some(d) => {
                    let spec: TaskSpec =
                        serde_json::from_str(&d).map_err(StoreError::Serialization)?;
                    Ok(Some(spec))
                }
                None => Ok(None),
            }
        }
    }

    fn update_status(
        &self,
        id: &str,
        status: TaskStatus,
    ) -> impl std::future::Future<Output = Result<TaskSpec, StoreError>> + Send {
        async move {
            let now = now_rfc3339();
            let status_val = status_str(status);
            let conn = self.conn.lock().expect("store mutex poisoned");
            // Сначала читаем данные
            let data: String = conn
                .query_row("SELECT data FROM tasks WHERE id = ?1", [id], |row| {
                    row.get(0)
                })
                .optional()
                .map_err(|e| StoreError::Sqlite(e.to_string()))?
                .ok_or_else(|| StoreError::NotFound(id.to_owned()))?;
            let spec: TaskSpec = serde_json::from_str(&data).map_err(StoreError::Serialization)?;
            // Обновляем статус
            conn.execute(
                "UPDATE tasks SET status = ?2, updated_at = ?3 WHERE id = ?1",
                [id, status_val, &now],
            )
            .map_err(|e| StoreError::Sqlite(e.to_string()))?;
            Ok(spec)
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
            let specs: Vec<TaskSpec> = stmt
                .query_map([status_str(status)], |row| {
                    let d: String = row.get(0)?;
                    serde_json::from_str(&d).map_err(|_e| rusqlite::Error::QueryReturnedNoRows)
                })
                .map_err(|e| StoreError::Sqlite(e.to_string()))?
                .filter_map(|r| r.transpose())
                .collect::<Result<Vec<_>, _>>()?;
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
            let specs: Vec<TaskSpec> = stmt
                .query_map([workspace_id], |row| {
                    let d: String = row.get(0)?;
                    serde_json::from_str(&d).map_err(|_e| rusqlite::Error::QueryReturnedNoRows)
                })
                .map_err(|e| StoreError::Sqlite(e.to_string()))?
                .filter_map(|r| r.transpose())
                .collect::<Result<Vec<_>, _>>()?;
            Ok(specs)
        }
    }
}

impl EventStore for SqliteStore {
    fn append(
        &self,
        _event: Event,
    ) -> impl std::future::Future<Output = Result<(), StoreError>> + Send {
        async {
            todo!("RED: SqliteStore::append — INSERT в events, task_id выделен в колонку")
        }
    }

    fn list_since(
        &self,
        _ts: &str,
    ) -> impl std::future::Future<Output = Result<Vec<Event>, StoreError>> + Send {
        async { todo!("RED: SqliteStore::list_since — SELECT WHERE ts >= ?1 ORDER BY ts") }
    }

    fn list_by_task(
        &self,
        _task_id: &str,
    ) -> impl std::future::Future<Output = Result<Vec<Event>, StoreError>> + Send {
        async { todo!("RED: SqliteStore::list_by_task — SELECT по idx_events_task") }
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
