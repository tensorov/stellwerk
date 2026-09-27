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
        _spec: TaskSpec,
    ) -> impl std::future::Future<Output = Result<TaskSpec, StoreError>> + Send {
        async { todo!("RED: SqliteStore::create — INSERT в tasks, status из spec") }
    }

    fn get(
        &self,
        _id: &str,
    ) -> impl std::future::Future<Output = Result<Option<TaskSpec>, StoreError>> + Send {
        async { todo!("RED: SqliteStore::get — SELECT data по id") }
    }

    fn update_status(
        &self,
        _id: &str,
        _status: TaskStatus,
    ) -> impl std::future::Future<Output = Result<TaskSpec, StoreError>> + Send {
        async {
            todo!("RED: SqliteStore::update_status — UPDATE status + data, NotFound при отсутствии")
        }
    }

    fn list_by_status(
        &self,
        _status: TaskStatus,
    ) -> impl std::future::Future<Output = Result<Vec<TaskSpec>, StoreError>> + Send {
        async { todo!("RED: SqliteStore::list_by_status — SELECT по idx_tasks_status") }
    }

    fn list_by_workspace(
        &self,
        _workspace_id: &str,
    ) -> impl std::future::Future<Output = Result<Vec<TaskSpec>, StoreError>> + Send {
        async { todo!("RED: SqliteStore::list_by_workspace — SELECT по idx_tasks_workspace") }
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
