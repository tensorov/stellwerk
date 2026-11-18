---
id: V2/11
task_id: V2-11
milestone: V2
status: proposed
depends_on: [N1/08]
files_allowed: [crates/stellwerk-task/tests/ledger_sqlite.rs]
max_files: 3
max_diff_lines: 200
acceptance: cargo test -p stellwerk-task --test ledger_sqlite
---

# V2/11 — F-01: lock pre-P03 ledger compatibility

## Цель
Цель: V2-11 F-01: lock pre-P03 ledger compatibility

## Шаг 1 — падающий тест
RED: тесты `old_ledger_rows_read_with_none_bindings` и `old_ledger_rows_missing_bindings_read_as_none` падают до их добавления; production-код не меняется.

## Что делать
add a test in `crates/stellwerk-task/tests/ledger_sqlite.rs` that inserts a JSON row with explicit `null` for `recipe_digest`/`policy_digest`/`model_fingerprint`/`commit_sha` into `evidence_ledger`, reads it back through `SqliteEvidenceLedger::entries`, and asserts all four are `None`; assert the chain tail is computed. Add a second test that inserts a JSON row omitting all four binding fields, reads it through `SqliteEvidenceLedger::entries`, and asserts all four are `None` and the chain tail is computed. `deny_unknown_fields` rejects unknown keys, not missing `Option<T>` fields; serde's `missing_field` returns `None` for `Option<T>`. Must NOT change production code, add `#[serde(default)]`, rewrite old rows, or add a migration.

## Приёмка
`cargo test -p stellwerk-task --test ledger_sqlite`; `old_ledger_rows_read_with_none_bindings` passes for explicit `null` bindings; `old_ledger_rows_missing_bindings_read_as_none` passes for omitted fields; both assert the tail via `evaluate(...).chain_tail_hash` (construct a `RunOutcome` inline in the test; do not try to share a helper from `tests/evidence.rs`, since integration-test binaries are separate crates; `chain_tail` is private).

## QA
happy - both the explicit-null and omitted-field tests; failure - corrupt a non-optional field such as `kind` and assert `serde_json::from_str::<EvidenceEntry>` rejects it. Evidence `.omo/evidence/task-15-stellwerk-production-v2.md`.

## Запрещено
- Must NOT change production code, add `#[serde(default)]`, rewrite old rows, or add a migration.

## Ссылки
`crates/stellwerk-task/src/ledger_sqlite.rs:40-110` (`open`, private `read`, serde error at `:75`); `crates/stellwerk-core/src/contract.rs:489-538` (`EvidenceEntry`), `:514-515` (`deny_unknown_fields`), `:531-538` (four `Option` fields); `crates/stellwerk-task/tests/ledger_sqlite.rs`; `crates/stellwerk-task/src/evidence.rs:86-105` (P03 fields).
