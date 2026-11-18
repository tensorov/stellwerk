---
id: V2/07b
task_id: V2-07b
milestone: V2
status: proposed
depends_on: [V2/07a, 07a, N1/07, V1/03, V1/o01, V1/o03, V1/o04, V1/v03b]
files_allowed: [crates/stellwerk-task/src/evidence.rs, crates/stellwerk-task/tests/evidence.rs]
max_files: 3
max_diff_lines: 200
acceptance: cargo test -p stellwerk-task --test evidence
---

# V2/07b — F-08: add the refutation constructor and update the count test

## Цель
Цель: V2-07b F-08: add the refutation constructor and update the count test

## Шаг 1 — падающий тест
RED: `only_the_constructors_mint_entries` падает, пока `refutation_entry` не добавлен (счёт 3 -> 4); тест конструктора проверяет `command == basis`, `result_hash == sha256_hex(refuting_data)`, все четыре binding = `None`.

## Что делать
add `refutation_entry(task_id, id, basis, refuting_data, ts) -> Result<EvidenceEntry, RefutationError>` in `crates/stellwerk-task/src/evidence.rs`; it delegates validation to the existing `refutation()`/`Refutation`/`RefutationError`, constructs `kind: EvidenceKind::Refuted`, sets `command: refutation.basis().to_owned()`, `result_hash: sha256_hex(refutation.refuting_data())` via a private `sha256_hex` helper using `sha2::{Digest, Sha256}` (already a dependency), and leaves `recipe_digest`, `policy_digest`, `model_fingerprint`, and `commit_sha` as `None`. Update `only_the_constructors_mint_entries` in `tests/evidence.rs` from 3 to 4 occurrences and add `Refuted` to the per-kind loop. Must NOT edit `tests/refutation.rs` here; that is todo 11. Must NOT reuse `Validation` for a negative decision.

## Приёмка
`cargo test -p stellwerk-task --test evidence`; the count test asserts 4 `kind: EvidenceKind::` occurrences and exactly one `Refuted`; a constructor test asserts `command == basis`, `result_hash == sha256_hex(refuting_data)`, and all four binding fields are `None`; a missing basis/refuting data returns `RefutationError`.

## QA
happy - the count test and the constructor test; failure - `refutation_entry` with an empty basis is rejected. Evidence `.omo/evidence/task-10-stellwerk-production-v2.md`.

## Запрещено
- Must NOT edit `tests/refutation.rs` here
- Must NOT reuse `Validation` for a negative decision.

## Ссылки
`crates/stellwerk-task/src/evidence.rs:160-215` (`Refutation`, `RefutationError`); `crates/stellwerk-task/src/evidence.rs:284` (`evaluate`); `crates/stellwerk-task/src/evidence.rs:441` (`kind_name`); `crates/stellwerk-task/tests/evidence.rs:450-468` (`only_the_constructors_mint_entries`).
