# Task 15 — V2-11 (F-01): lock pre-P03 ledger compatibility

- Packet: `.sisyphus/plans/V2/packet-11.md` (V2/11, task_id V2-11)
- Commit: `d933d708a06bae2a7bcc1a33c685b6afa46ea100` — `test(task): lock pre-P03 ledger compatibility`
- Files: `crates/stellwerk-task/tests/ledger_sqlite.rs` only (155 insertions, 2 deletions)
- New tests: `old_ledger_rows_read_with_none_bindings`, `old_ledger_rows_missing_bindings_read_as_none`
- C2 (`scripts/plan-lint.py::added_counts` on `git show d933d70 --unified=0`): **src 0 / tests 155** (limits 200/250) — `budget_violations` empty.

## RED

Command before the tests existed:

```
$ cargo test -p stellwerk-task --test ledger_sqlite old_ledger
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/ledger_sqlite.rs (target/debug/deps/ledger_sqlite-a00faed65cbd5fe2)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out; finished in 0.00s

EXIT=0
```

Honest note on RED: a behavioural RED is impossible for this compatibility-lock
packet without changing production code, which the packet forbids. The tests lock
behaviour that is already correct by construction (`Option<T>` missing/null ->
`None`), and the failing path being locked is the panic in
`SqliteEvidenceLedger::entries` (`crates/stellwerk-task/src/ledger_sqlite.rs:102-111`)
when serde rejects a row. The observed RED is therefore "0 tests matched / 0 passed",
not a failing assertion. The negative assertion (`kind: "NotAKind"` -> serde error)
is the executable guard that the compatibility tests are not passing vacuously.

## GREEN

### `cargo test -p stellwerk-task --test ledger_sqlite`

```
   Compiling stellwerk-task v0.0.1 (/home/izyabretatel/gits/stellwerk/crates/stellwerk-task)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.39s
     Running tests/ledger_sqlite.rs (target/debug/deps/ledger_sqlite-a00faed65cbd5fe2)

running 7 tests
test entry_without_validation_reads_back_as_awaiting_human ... ok
test storage_failure_is_returned_not_lost ... ok
test entries_come_back_in_insertion_order ... ok
test entries_for_unknown_task_is_empty_not_everything ... ok
test old_ledger_rows_missing_bindings_read_as_none ... ok
test old_ledger_rows_read_with_none_bindings ... ok
test ledger_persists_across_reopen ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

EXIT=0
```

### `cargo test -p stellwerk-task`

Exit 0, 43 `test result:` lines, all `ok` with `0 failed`:

```
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.33s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.87s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### `cargo clippy -p stellwerk-task --all-targets -- -D warnings`

```
    Checking stellwerk-core v0.0.1 (/home/izyabretatel/gits/stellwerk/crates/stellwerk-core)
    Checking stellwerk-store v0.0.1 (/home/izyabretatel/gits/stellwerk/crates/stellwerk-store)
    Checking stellwerk-gateway v0.0.1 (/home/izyabretatel/gits/stellwerk/crates/stellwerk-gateway)
    Checking stellwerk-task v0.0.1 (/home/izyabretatel/gits/stellwerk/crates/stellwerk-task)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.02s
CLIPPY_EXIT=0
```

### `cargo fmt --all -- --check`

```
FMT_EXIT=0
```

### `git show --numstat d933d70`

```
d933d70 test(task): lock pre-P03 ledger compatibility
155	2	crates/stellwerk-task/tests/ledger_sqlite.rs
```

## How the raw rows were produced

`SqliteEvidenceLedger::open(&scratch.db())` creates the schema; that handle is
dropped, then a raw `rusqlite::Connection::open(scratch.db())` inserts directly
into `evidence_ledger (task_id, id, data)` with a hand-built `serde_json::json!`
row. The ledger is then reopened and read through `SqliteEvidenceLedger::entries`.
Expected tail: `evaluate(&outcome, &[model_claim(TASK, id, result_hash, TS)])`
with `RunOutcome` constructed inline in the test (`chain_tail` is private).

## Diff

```diff
commit d933d708a06bae2a7bcc1a33c685b6afa46ea100
Author: tensorov <aleksei.v.molchanov@gmail.com>
Date:   Wed Nov 18 01:43:37 2026 +0100

    test(task): lock pre-P03 ledger compatibility

diff --git a/crates/stellwerk-task/tests/ledger_sqlite.rs b/crates/stellwerk-task/tests/ledger_sqlite.rs
index 828e948..1d0abe7 100644
--- a/crates/stellwerk-task/tests/ledger_sqlite.rs
+++ b/crates/stellwerk-task/tests/ledger_sqlite.rs
@@ -13,11 +13,15 @@
 use std::path::PathBuf;
 use std::sync::atomic::{AtomicU64, Ordering};
 
+use rusqlite::Connection;
+use serde_json::json;
+
 use stellwerk_task::evidence::{
-    evidence_status, model_claim, tool_attestation, EvidenceKind, EvidenceLedger, EvidenceStatus,
-    LedgerError,
+    evaluate, evidence_status, model_claim, tool_attestation, EvidenceEntry, EvidenceKind,
+    EvidenceLedger, EvidenceStatus, LedgerError,
 };
 use stellwerk_task::ledger_sqlite::SqliteEvidenceLedger;
+use stellwerk_task::runner::{RunOutcome, StopReason, Verdict};
 
 const TASK: &str = "t-ledger";
 /// Время фиксировано и одинаково у обеих записей теста порядка: порядок должен
@@ -209,3 +213,152 @@ fn ledger_persists_across_reopen() {
         "записи пережили переоткрытие базы и сохранили порядок"
     );
 }
+
+/// Хвост, посчитанный по цепочке, прочитанной из SQLite, обязан совпасть с
+/// хвостом той же записи, собранной в памяти. `ts` в хвост не входит, поэтому
+/// совпадение проверяет именно восстановление полей, а не время.
+fn expected_tail(entry: EvidenceEntry) -> String {
+    let outcome = RunOutcome {
+        stop_reason: StopReason::Stop,
+        tool_steps: 2,
+        turns: 3,
+        total_input_tokens: 1_000,
+        verdict: Verdict::Valid,
+    };
+    evaluate(&outcome, &[entry]).chain_tail_hash
+}
+
+/// Запись, снятая до P03, несёт явные `null` в четырёх binding-полях. Чтение
+/// обязано вернуть `None`, а не отвергнуть строку: `entries` паникует на ошибке
+/// десериализации, и паника на старой записи сделала бы ledger непригодным после
+/// обновления (совместимость — часть контракта хранилища).
+#[test]
+fn old_ledger_rows_read_with_none_bindings() {
+    let scratch = Scratch::new("pre-p03-null");
+    drop(open(&scratch));
+
+    let legacy = json!({
+        "id": "legacy-null",
+        "task_id": TASK,
+        "kind": "ModelClaim",
+        "command": "",
+        "result_hash": "h-legacy-null",
+        "ts": TS,
+        "recipe_digest": null,
+        "policy_digest": null,
+        "model_fingerprint": null,
+        "commit_sha": null,
+    })
+    .to_string();
+    let conn = Connection::open(scratch.db()).expect("raw sqlite connection opens");
+    conn.execute(
+        "INSERT INTO evidence_ledger (task_id, id, data) VALUES (?1, ?2, ?3)",
+        rusqlite::params![TASK, "legacy-null", legacy],
+    )
+    .expect("pre-P03 row inserted");
+    drop(conn);
+
+    let ledger = open(&scratch);
+    let chain = ledger.entries(TASK);
+    assert_eq!(
+        chain.len(),
+        1,
+        "старая запись прочитана, а не отвергнута: {chain:?}"
+    );
+    let entry = &chain[0];
+    assert_eq!(entry.recipe_digest, None, "явный null рецепта — это None");
+    assert_eq!(entry.policy_digest, None, "явный null политики — это None");
+    assert_eq!(
+        entry.model_fingerprint, None,
+        "явный null отпечатка модели — это None"
+    );
+    assert_eq!(entry.commit_sha, None, "явный null коммита — это None");
+
+    let expected = expected_tail(model_claim(TASK, "legacy-null", "h-legacy-null", TS));
+    let tail = expected_tail(entry.clone());
+    assert!(
+        !tail.is_empty(),
+        "хвост посчитан для прочитанной старой записи, а не оставлен пустым"
+    );
+    assert_eq!(
+        tail, expected,
+        "хвост старой записи совпадает с хвостом той же записи, собранной в памяти"
+    );
+}
+
+/// Запись, снятая до P03, может вовсе не содержать четырёх binding-ключей:
+/// `deny_unknown_fields` запрещает лишние ключи, но отсутствие `Option<T>` — это
+/// `None`, а не ошибка. Второй тест закрывает именно этот путь и заодно
+/// фиксирует, что испорченное обязательное поле по-прежнему отвергается.
+#[test]
+fn old_ledger_rows_missing_bindings_read_as_none() {
+    let scratch = Scratch::new("pre-p03-missing");
+    drop(open(&scratch));
+
+    let legacy = json!({
+        "id": "legacy-missing",
+        "task_id": TASK,
+        "kind": "ModelClaim",
+        "command": "",
+        "result_hash": "h-legacy-missing",
+        "ts": TS,
+    })
+    .to_string();
+    let conn = Connection::open(scratch.db()).expect("raw sqlite connection opens");
+    conn.execute(
+        "INSERT INTO evidence_ledger (task_id, id, data) VALUES (?1, ?2, ?3)",
+        rusqlite::params![TASK, "legacy-missing", legacy],
+    )
+    .expect("pre-P03 row without bindings inserted");
+    drop(conn);
+
+    let ledger = open(&scratch);
+    let chain = ledger.entries(TASK);
+    assert_eq!(
+        chain.len(),
+        1,
+        "старая запись без binding-ключей прочитана, а не отвергнута: {chain:?}"
+    );
+    let entry = &chain[0];
+    assert_eq!(
+        entry.recipe_digest, None,
+        "отсутствующий ключ рецепта — это None, а не ошибка"
+    );
+    assert_eq!(
+        entry.policy_digest, None,
+        "отсутствующий ключ политики — это None, а не ошибка"
+    );
+    assert_eq!(
+        entry.model_fingerprint, None,
+        "отсутствующий ключ отпечатка модели — это None, а не ошибка"
+    );
+    assert_eq!(
+        entry.commit_sha, None,
+        "отсутствующий ключ коммита — это None, а не ошибка"
+    );
+
+    let expected = expected_tail(model_claim(TASK, "legacy-missing", "h-legacy-missing", TS));
+    let tail = expected_tail(entry.clone());
+    assert!(
+        !tail.is_empty(),
+        "хвост посчитан для прочитанной старой записи без bindings"
+    );
+    assert_eq!(
+        tail, expected,
+        "хвост записи без binding-ключей совпадает с хвостом той же записи в памяти"
+    );
+
+    let corrupted = json!({
+        "id": "corrupt",
+        "task_id": TASK,
+        "kind": "NotAKind",
+        "command": "",
+        "result_hash": "h-corrupt",
+        "ts": TS,
+    })
+    .to_string();
+    assert!(
+        serde_json::from_str::<EvidenceEntry>(&corrupted).is_err(),
+        "испорченный обязательный kind обязан быть отвергнут, а не прочитан как запись"
+    );
+}
```

## PolyForge chain (V2/packet-11)

- Product commit: `d933d708a06bae2a7bcc1a33c685b6afa46ea100` (`test(task): lock pre-P03 ledger compatibility`), 1 file, 155/2 lines.
- `diff_hash`: `98e4d4bdf81782e2bdea1e637c3ee364c7448ef92c6c2753c1eb9c9075971945`
- `evidence_append(ModelClaim)`: entry 402, hash `e6e87ffe46bf56202fda7dc7befeb8298350f55289ace2d09b3f76c4a69f8eca`, state `ModelClaimed`.
- `evidence_verify(cargo test -p stellwerk-task --test ledger_sqlite)`: entry 403, exit 0, state `Verified`, stdout_hash `51b152506362fe6a6055d220bc8ae74889ca9e05c5c3124b765b8a0738997e6e`, hash `a8394166ac3abeeb907bb0b1eb877de89aaa2fc730559b2fd7b1ef91d3e32b6a`.
- `gate_evaluate(required=[Verified])`: `passed=true`, verified=1, claimed=1, validated=0, missing=[].
- `chain_tail`: `a8394166ac3abeeb907bb0b1eb877de89aaa2fc730559b2fd7b1ef91d3e32b6a`

Orchestrator independent verification: `git show d933d70` reviewed line by line (two tests, raw `rusqlite` row insert, four `None` assertions, tail equality against `evaluate(&outcome, &[model_claim(...)])`, `kind: "NotAKind"` rejection); `cargo test -p stellwerk-task --test ledger_sqlite` -> 7 passed 0 failed; `cargo test -p stellwerk-task` -> 43 result lines ok, no failures; `cargo clippy -p stellwerk-task --all-targets -- -D warnings` exit 0; `cargo fmt --all -- --check` exit 0; C2 tests 155/250, src 0/200.
