//! N1-08: цепочка доказательств на отдельной таблице SQLite.
//!
//! Четыре требования, и каждое ломается по-своему. Порядок обязателен:
//! `chain_tail` не зависит от порядка и потому не защищает от перестановки
//! доказательств — защищает именно `entries`. Чужой `task_id` обязан вернуть
//! пусто, а не «все записи». Отказ хранилища обязан ВЕРНУТЬСЯ: запись, о которой
//! никто не знает, и запись, которую отвергли, выглядят одинаково (закон 6).
//! Запись без `Validation` читается как «ждёт человека», а не `Done` (решение 5).
//!
//! Импорт идёт через публичный API крейта: `#[path]` собрал бы вторую копию
//! контракта, и расхождение обнаружилось бы в чужом коде.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use stellwerk_task::evidence::{
    evidence_status, model_claim, tool_attestation, EvidenceKind, EvidenceLedger, EvidenceStatus,
    LedgerError,
};
use stellwerk_task::ledger_sqlite::SqliteEvidenceLedger;

const TASK: &str = "t-ledger";
/// Время фиксировано и одинаково у обеих записей теста порядка: порядок должен
/// приходить из последовательности вставки, а не из `ts`.
const TS: &str = "2026-10-12T00:00:00Z";
const TEST_CMD: &str = "cargo test -p stellwerk-task --test ledger_sqlite";

/// Уникальный каталог на тест: без изоляции параллельные тесты делили бы одну
/// базу и загрязняли порядок записей друг другу.
static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

/// Временный каталог под базу. Удаляется в `Drop`, чтобы после прогона не
/// осталось посторонних SQLite-файлов.
struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "stellwerk-ledger-{}-{}-{}",
            tag,
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).expect("scratch dir создаётся");
        Scratch { dir }
    }

    fn db(&self) -> PathBuf {
        self.dir.join("ledger.sqlite")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn open(scratch: &Scratch) -> SqliteEvidenceLedger {
    SqliteEvidenceLedger::open(&scratch.db()).expect("ledger открывается")
}

#[test]
fn entries_come_back_in_insertion_order() {
    let scratch = Scratch::new("order");
    let mut ledger = open(&scratch);

    let first = model_claim(TASK, "z-first", "h-1", TS);
    let second = model_claim(TASK, "a-second", "h-2", TS);
    ledger.append(&first).expect("первая запись принята");
    ledger.append(&second).expect("вторая запись принята");

    let read = ledger.entries(TASK);
    assert_eq!(read.len(), 2, "обе записи прочитаны: {read:?}");
    assert_eq!(
        read[0].id, "z-first",
        "первая добавленная запись идёт первой"
    );
    assert_eq!(
        read[1].id, "a-second",
        "вторая добавленная идёт второй, хотя её id лексикографически меньше — порядок по вставке, а не по id"
    );
    assert_eq!(
        read,
        vec![first, second],
        "содержимое и порядок совпадают с порядком записи"
    );
}

#[test]
fn entries_for_unknown_task_is_empty_not_everything() {
    let scratch = Scratch::new("foreign");
    let mut ledger = open(&scratch);

    ledger
        .append(&model_claim(TASK, "e-a", "h-a", TS))
        .expect("запись задачи A принята");
    ledger
        .append(&model_claim("task-b", "e-b", "h-b", TS))
        .expect("запись задачи B принята");

    let unknown = ledger.entries("task-c");
    assert!(
        unknown.is_empty(),
        "чужая задача не видит чужих записей, а не «все записи»: {unknown:?}"
    );
    assert_eq!(
        ledger.entries(TASK).len(),
        1,
        "своя задача видит ровно свою запись"
    );
    assert_eq!(
        ledger.entries("task-b").len(),
        1,
        "вторая задача видит ровно свою запись"
    );
}

#[test]
fn storage_failure_is_returned_not_lost() {
    let scratch = Scratch::new("failure");
    let mut ledger = open(&scratch);

    let entry = model_claim(TASK, "dup-id", "h", TS);
    ledger.append(&entry).expect("первая запись принята");

    let err = ledger
        .append(&entry)
        .expect_err("повтор id — отказ хранилища, а не тихий успех");
    match err {
        LedgerError::Unavailable { reason } => {
            assert!(
                !reason.trim().is_empty(),
                "причина отказа сохранена дословно, иначе расследование получает выдуманный ответ: {reason:?}"
            );
        }
    }
    assert_eq!(
        ledger.entries(TASK).len(),
        1,
        "отвергнутая запись не появилась в цепочке: отказ не превратился в запись"
    );
}

#[test]
fn entry_without_validation_reads_back_as_awaiting_human() {
    let scratch = Scratch::new("awaiting");
    let mut ledger = open(&scratch);

    ledger
        .append(&model_claim(TASK, "e-claim", "h-claim", TS))
        .expect("утверждение модели принято");
    ledger
        .append(&tool_attestation(TASK, "e-tool", TEST_CMD, "h-tool", TS))
        .expect("удостоверение инструмента принято");

    let chain = ledger.entries(TASK);
    assert_eq!(chain.len(), 2, "обе записи вернулись: {chain:?}");
    assert_eq!(
        chain.iter().map(|entry| entry.kind).collect::<Vec<_>>(),
        vec![EvidenceKind::ModelClaim, EvidenceKind::ToolAttestation],
        "вид записи переживает сериализацию без подмены"
    );
    assert_eq!(
        chain[1].command, TEST_CMD,
        "команда удостоверения прочитана дословно"
    );

    let status = evidence_status(&chain);
    assert_eq!(
        status,
        EvidenceStatus::AwaitingHumanValidation,
        "удостоверение есть, решения человека нет — результат ждёт человека"
    );
    assert_ne!(
        status,
        EvidenceStatus::HumanValidated,
        "без Validation результат не принят"
    );
    assert_ne!(
        status.label(),
        "Done",
        "Done не объявляется до решения человека (решение 5)"
    );
}

#[test]
fn ledger_persists_across_reopen() {
    let scratch = Scratch::new("persist");
    {
        let mut ledger = open(&scratch);
        ledger
            .append(&model_claim(TASK, "e-1", "h-1", TS))
            .expect("запись принята");
        ledger
            .append(&model_claim(TASK, "e-2", "h-2", TS))
            .expect("запись принята");
    }

    let reopened = open(&scratch);
    let read = reopened.entries(TASK);
    assert_eq!(
        read.iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        vec!["e-1", "e-2"],
        "записи пережили переоткрытие базы и сохранили порядок"
    );
}
