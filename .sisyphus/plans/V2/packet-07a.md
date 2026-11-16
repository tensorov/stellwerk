---
id: V2/07a
task_id: V2-07a
milestone: V2
status: proposed
depends_on: [M1/05, M1/06, N1/07, V1/03, V1/o01, V1/o03, V1/o04, V1/v03b]
files_allowed: [crates/stellwerk-core/src/contract.rs, crates/stellwerk-task/src/evidence.rs, docs/adr/0024-refutation-evidence-kind.md]
max_files: 3
max_diff_lines: 200
acceptance: cargo test -p stellwerk-core
contract-change: true
---

# V2/07a — F-08: add the Refuted evidence kind (contract-change)

## Цель
Цель: V2-07a F-08: add the Refuted evidence kind (contract-change)
Пакет помечен `contract-change`: правка `crates/stellwerk-core/src/contract.rs` требует ADR в `docs/adr/`.

## Шаг 1 — падающий тест
RED: serde round-trip тест `EvidenceKind::Refuted` в `#[cfg(test)]` модуле `contract.rs` падает до добавления варианта; `cargo check -p stellwerk-task` показывает неполные `match`.

## Что делать
add `EvidenceKind::Refuted` to `crates/stellwerk-core/src/contract.rs`; update every exhaustive `match` on `EvidenceKind` so `cargo check -p stellwerk-task` compiles: `kind_name` (`crates/stellwerk-task/src/evidence.rs:441`) returns `"Refuted"`, and `evaluate`'s `match entry.kind` (`crates/stellwerk-task/src/evidence.rs:284` region) gets a `Refuted => {}` arm (todo 11 implements the semantics); add `docs/adr/0024-refutation-evidence-kind.md` explaining why a `Refuted` chain entry is not a `Validation` and not a `Failed` run, and explicitly state that it supersedes the `evidence.rs:160-165` comment ("not a new `EvidenceKind`") and reconciles ADR-0022's `TaskStatus::Refuted`. Add the serde round-trip test in a `#[cfg(test)]` module at the bottom of `contract.rs`. The packet carries the `contract-change` label. Must NOT reuse `Validation` for a negative decision. Must NOT change `TaskStatus::Refuted`. Must NOT add `refutation_entry` here; that is todo 10.

## Приёмка
`cargo test -p stellwerk-core`; `cargo check -p stellwerk-task`; `test -f docs/adr/0024-refutation-evidence-kind.md`; `rg -n "contract-change" .sisyphus/plans/V2/packet-07a.md` (the packet frontmatter).

## QA
happy - the new kind round-trips through `serde_json`; failure - an unknown kind still fails deserialization. Evidence `.omo/evidence/task-9-stellwerk-production-v2.md`.

## Запрещено
- Must NOT reuse `Validation` for a negative decision.
- Must NOT change `TaskStatus::Refuted`.
- Must NOT add `refutation_entry` here

## Ссылки
`crates/stellwerk-core/src/contract.rs:479-486` (`EvidenceKind`); `crates/stellwerk-core/src/contract.rs:489-538` (`EvidenceEntry`); `crates/stellwerk-task/src/evidence.rs:160-215` (`Refutation`); `crates/stellwerk-task/src/evidence.rs:284` (`evaluate`); `crates/stellwerk-task/src/evidence.rs:441` (`kind_name`); `docs/adr/0022-task-status-human-and-refutation.md`; `docs/adr/0006-riegel-attestation-exception.md` (contract-change precedent).
