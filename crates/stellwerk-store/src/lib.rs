//! stellwerk-store: трейты хранения и их SQLite-реализация.
//!
//! Трейты задают **единственный** способ, которым остальной федерации разрешено
//! читать и писать состояние. Это делает будущую замену SQLite на Postgres
//! изменением реализации, а не вызовов: см. `docs/AX_INSPIRATION.md` §7.
//!
//! Модель данных намеренно простая: `id TEXT PRIMARY KEY`, `data` — JSON,
//! `created_at`/`updated_at` — текст. Колонки для индексов добавляются позже
//! миграциями, а не переписыванием типов.
//!
//! ## Состояние реализации
//!
//! Трейты готовы. `SqliteStore` — **RED-спецификация**: тела методов `todo!()`,
//! тесты в `tests/` компилируются и падают. См. `.night/STATE.md` (фаза 4).

pub mod migrations;
pub mod sqlite;
pub mod traits;

pub use sqlite::SqliteStore;
pub use traits::{EventStore, ModuleRecord, ModuleStore, StoreError, TaskStore};
