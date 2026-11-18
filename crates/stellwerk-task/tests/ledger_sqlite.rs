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

use rusqlite::Connection;
use serde_json::json;

use stellwerk_task::evidence::{
    evaluate, evidence_status, model_claim, tool_attestation, EvidenceEntry, EvidenceKind,
    EvidenceLedger, EvidenceStatus, LedgerError,
};
use stellwerk_task::ledger_sqlite::SqliteEvidenceLedger;
use stellwerk_task::runner::{RunOutcome, StopReason, Verdict};

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

/// Хвост, посчитанный по цепочке, прочитанной из SQLite, обязан совпасть с
/// хвостом той же записи, собранной в памяти. `ts` в хвост не входит, поэтому
/// совпадение проверяет именно восстановление полей, а не время.
fn expected_tail(entry: EvidenceEntry) -> String {
    let outcome = RunOutcome {
        stop_reason: StopReason::Stop,
        tool_steps: 2,
        turns: 3,
        total_input_tokens: 1_000,
        verdict: Verdict::Valid,
    };
    evaluate(&outcome, &[entry]).chain_tail_hash
}

/// Запись, снятая до P03, несёт явные `null` в четырёх binding-полях. Чтение
/// обязано вернуть `None`, а не отвергнуть строку: `entries` паникует на ошибке
/// десериализации, и паника на старой записи сделала бы ledger непригодным после
/// обновления (совместимость — часть контракта хранилища).
#[test]
fn old_ledger_rows_read_with_none_bindings() {
    let scratch = Scratch::new("pre-p03-null");
    drop(open(&scratch));

    let legacy = json!({
        "id": "legacy-null",
        "task_id": TASK,
        "kind": "ModelClaim",
        "command": "",
        "result_hash": "h-legacy-null",
        "ts": TS,
        "recipe_digest": null,
        "policy_digest": null,
        "model_fingerprint": null,
        "commit_sha": null,
    })
    .to_string();
    let conn = Connection::open(scratch.db()).expect("raw sqlite connection opens");
    conn.execute(
        "INSERT INTO evidence_ledger (task_id, id, data) VALUES (?1, ?2, ?3)",
        rusqlite::params![TASK, "legacy-null", legacy],
    )
    .expect("pre-P03 row inserted");
    drop(conn);

    let ledger = open(&scratch);
    let chain = ledger.entries(TASK);
    assert_eq!(
        chain.len(),
        1,
        "старая запись прочитана, а не отвергнута: {chain:?}"
    );
    let entry = &chain[0];
    assert_eq!(entry.recipe_digest, None, "явный null рецепта — это None");
    assert_eq!(entry.policy_digest, None, "явный null политики — это None");
    assert_eq!(
        entry.model_fingerprint, None,
        "явный null отпечатка модели — это None"
    );
    assert_eq!(entry.commit_sha, None, "явный null коммита — это None");

    let expected = expected_tail(model_claim(TASK, "legacy-null", "h-legacy-null", TS));
    let tail = expected_tail(entry.clone());
    assert!(
        !tail.is_empty(),
        "хвост посчитан для прочитанной старой записи, а не оставлен пустым"
    );
    assert_eq!(
        tail, expected,
        "хвост старой записи совпадает с хвостом той же записи, собранной в памяти"
    );
}

/// Запись, снятая до P03, может вовсе не содержать четырёх binding-ключей:
/// `deny_unknown_fields` запрещает лишние ключи, но отсутствие `Option<T>` — это
/// `None`, а не ошибка. Второй тест закрывает именно этот путь и заодно
/// фиксирует, что испорченное обязательное поле по-прежнему отвергается.
#[test]
fn old_ledger_rows_missing_bindings_read_as_none() {
    let scratch = Scratch::new("pre-p03-missing");
    drop(open(&scratch));

    let legacy = json!({
        "id": "legacy-missing",
        "task_id": TASK,
        "kind": "ModelClaim",
        "command": "",
        "result_hash": "h-legacy-missing",
        "ts": TS,
    })
    .to_string();
    let conn = Connection::open(scratch.db()).expect("raw sqlite connection opens");
    conn.execute(
        "INSERT INTO evidence_ledger (task_id, id, data) VALUES (?1, ?2, ?3)",
        rusqlite::params![TASK, "legacy-missing", legacy],
    )
    .expect("pre-P03 row without bindings inserted");
    drop(conn);

    let ledger = open(&scratch);
    let chain = ledger.entries(TASK);
    assert_eq!(
        chain.len(),
        1,
        "старая запись без binding-ключей прочитана, а не отвергнута: {chain:?}"
    );
    let entry = &chain[0];
    assert_eq!(
        entry.recipe_digest, None,
        "отсутствующий ключ рецепта — это None, а не ошибка"
    );
    assert_eq!(
        entry.policy_digest, None,
        "отсутствующий ключ политики — это None, а не ошибка"
    );
    assert_eq!(
        entry.model_fingerprint, None,
        "отсутствующий ключ отпечатка модели — это None, а не ошибка"
    );
    assert_eq!(
        entry.commit_sha, None,
        "отсутствующий ключ коммита — это None, а не ошибка"
    );

    let expected = expected_tail(model_claim(TASK, "legacy-missing", "h-legacy-missing", TS));
    let tail = expected_tail(entry.clone());
    assert!(
        !tail.is_empty(),
        "хвост посчитан для прочитанной старой записи без bindings"
    );
    assert_eq!(
        tail, expected,
        "хвост записи без binding-ключей совпадает с хвостом той же записи в памяти"
    );

    let corrupted = json!({
        "id": "corrupt",
        "task_id": TASK,
        "kind": "NotAKind",
        "command": "",
        "result_hash": "h-corrupt",
        "ts": TS,
    })
    .to_string();
    assert!(
        serde_json::from_str::<EvidenceEntry>(&corrupted).is_err(),
        "испорченный обязательный kind обязан быть отвергнут, а не прочитан как запись"
    );
}
