//! # SQLite-реализация `EvidenceLedger`
//!
//! Отдельная таблица `evidence_ledger`, а не `tasks/events/modules` из
//! `stellwerk-store`. Разделение обязательно: цепочка доказательств делает
//! «проверенную запись» проверяемой, и отказ её хранилища не имеет права ронять
//! event-лог. Схема описана здесь, `sqlite.rs` соседнего крейта не трогается.
//!
//! Порядок записей — по монотонной последовательности вставки (`seq`), а не по
//! `ts` (две записи могут делить время) и не по `id` (порядок вставки не обязан
//! совпадать с лексикографическим). `chain_tail` от порядка не зависит, поэтому
//! от перестановки доказательств защищает именно `entries`.
//!
//! `append` отказ хранилища **возвращает** через `LedgerError::Unavailable`, а не
//! теряет: запись, о которой никто не знает, и запись, которую отвергли, иначе
//! выглядят одинаково (закон 6).

use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;

use crate::evidence::{EvidenceEntry, EvidenceLedger, LedgerError};

/// Схема отдельной таблицы доказательств. `seq` — `AUTOINCREMENT`, чтобы
/// удалённая запись не позволила переиспользовать её номер и новый факт не встал
/// на место старого. `id` уникален: две записи с одним id — не две записи, и
/// повтор отвергается хранилищем, а не тихо дублируется.
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS evidence_ledger (
    seq     INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id TEXT NOT NULL,
    id      TEXT NOT NULL UNIQUE,
    data    TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS evidence_ledger_task_seq
    ON evidence_ledger (task_id, seq);
";

/// Цепочка доказательств на SQLite. `Mutex<Connection>` — как и в
/// `stellwerk-store`: соединение rusqlite не `Sync`, а одна блокировка на всё
/// соединение — сознательная уступка простоте, федерация пишет мало.
pub struct SqliteEvidenceLedger {
    conn: Mutex<Connection>,
}

impl SqliteEvidenceLedger {
    /// Открывает базу и создаёт таблицу, если её ещё нет. `path` может быть
    /// `:memory:`: для тестов и dry-run.
    pub fn open(path: &Path) -> Result<Self, LedgerError> {
        let conn = Connection::open(path).map_err(unavailable)?;
        Self::from_connection(conn)
    }

    fn from_connection(conn: Connection) -> Result<Self, LedgerError> {
        conn.execute_batch(SCHEMA).map_err(unavailable)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Записи задачи в порядке вставки. Отдельный fallible-метод: трейт
    /// `entries` не возвращает `Result`, поэтому отказ чтения здесь ещё можно
    /// выразить, а в трейте — уже нет.
    fn read(&self, task_id: &str) -> Result<Vec<EvidenceEntry>, LedgerError> {
        let conn = self.conn.lock().expect("evidence ledger mutex poisoned");
        let mut stmt = conn
            .prepare("SELECT data FROM evidence_ledger WHERE task_id = ?1 ORDER BY seq")
            .map_err(unavailable)?;
        let rows = stmt
            .query_map([task_id], |row| row.get::<_, String>(0))
            .map_err(unavailable)?;
        let mut entries = Vec::new();
        for row in rows {
            let data = row.map_err(unavailable)?;
            entries.push(serde_json::from_str(&data).map_err(unavailable)?);
        }
        Ok(entries)
    }
}

/// Превращает любую ошибку хранилища в `LedgerError::Unavailable` с причиной
/// дословно. Своей обёртки над сообщением нет специально: выдуманный текст
/// причины дал бы расследованию выдуманный ответ.
fn unavailable(error: impl std::fmt::Display) -> LedgerError {
    LedgerError::Unavailable {
        reason: error.to_string(),
    }
}

impl EvidenceLedger for SqliteEvidenceLedger {
    fn append(&mut self, entry: &EvidenceEntry) -> Result<(), LedgerError> {
        let data = serde_json::to_string(entry).map_err(unavailable)?;
        let conn = self.conn.lock().expect("evidence ledger mutex poisoned");
        conn.execute(
            "INSERT INTO evidence_ledger (task_id, id, data) VALUES (?1, ?2, ?3)",
            rusqlite::params![&entry.task_id, &entry.id, data],
        )
        .map(|_| ())
        .map_err(unavailable)
    }

    fn entries(&self, task_id: &str) -> Vec<EvidenceEntry> {
        match self.read(task_id) {
            Ok(entries) => entries,
            // Трейт возвращает `Vec`, а не `Result`: вернуть пустой список при
            // отказе чтения значило бы выдать «доказательств нет» за отсутствие
            // записей. Молчание здесь неотличимо от сознательного ответа, поэтому
            // отказ чтения останавливает вызывающего громко (закон 6).
            Err(error) => panic!("evidence ledger read failed: {error}"),
        }
    }
}
