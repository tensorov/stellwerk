---
id: V2/07c
task_id: V2-07c
milestone: V2
status: proposed
depends_on: [V2/03, V2/07b, 03, 07b, N1/04, N1/07, N2/00b, V1/03, V1/o01, V1/o02, V1/o03, V1/o04, V1/q01, V1/q02, V1/s01, V1/s02, V1/s03, V1/v03b]
files_allowed: [crates/stellwerk-task/src/evidence.rs, crates/stellwerk-task/src/runner.rs, crates/stellwerk-task/tests/refutation.rs]
max_files: 3
max_diff_lines: 200
acceptance: cargo test -p stellwerk-task --test evidence --test refutation
---

# V2/07c — F-08: carry refutation into the evidence chain

## Цель
Цель: V2-07c F-08: carry refutation into the evidence chain

## Шаг 1 — падающий тест
RED: тест refuted-цепочки падает, пока `evidence_status`, `evaluate` и `run_once_resuming` не различают `Refuted`; счётный worker на refuted ledger даёт `ResumeOutcome::Refuted` и ноль вызовов.

## Что делать
update `evidence_status` to return `EvidenceStatus::Refuted` when the chain contains `EvidenceKind::Refuted`; check `Refuted` before `ToolAttestation` so a refuted-only chain is `Refuted`, not `Unattested`. Add the `EvidenceStatus::Refuted` arm to `EvidenceStatus::label` (`evidence.rs:233`). At the start of `evaluate`, after `let mut missing`, if `let Some(entry) = chain.iter().find(|entry| entry.kind == EvidenceKind::Refuted)`, push `format!("refuted: {}", entry.command)` and return `GateResult { passed: false, missing, verified: chain.len() as u64, chain_tail_hash: chain_tail(chain) }` before the ToolAttestation/Validation checks; this makes a refuted-only chain `Refuted`, not `AwaitingHumanValidation` or `Failed`. Add `ResumeOutcome::Refuted` and update `run_once_resuming` (todo 4) so its `EvidenceStatus` match has the explicit arm `EvidenceStatus::Refuted => Ok(ResumeOutcome::Refuted)` before the `EvidenceStatus::Unattested` fallthrough, returning without calling the worker; do not introduce a `Completed` variant and do not route `Refuted` through `Proceeded`/`run_once`. Put all new tests in `tests/refutation.rs`; do not edit `tests/evidence.rs`. Must NOT collapse `Refuted` into `Failed` or `AwaitingHumanValidation`. Must NOT use a fixture `Validation` to fake the refutation.

## Приёмка
`cargo test -p stellwerk-task --test evidence --test refutation`; a refuted chain yields `EvidenceStatus::Refuted` and `evaluate(...).passed == false` with `missing` containing `refuted: <basis>`; a `run_once_resuming` test with a refuted ledger and a counting worker returns `ResumeOutcome::Refuted` and records zero worker calls.

## QA
happy - the refuted-chain test; failure - a refutation without basis/refuting data is not constructible; a `Validation` with `FIXTURE_VALIDATION_LABEL` still does not count as human. Evidence `.omo/evidence/task-11-stellwerk-production-v2.md`.

## Запрещено
- do not introduce a `Completed` variant and do not route `Refuted` through `Proceeded`/`run_once`.
- do not edit `tests/evidence.rs`.
- Must NOT collapse `Refuted` into `Failed` or `AwaitingHumanValidation`.
- Must NOT use a fixture `Validation` to fake the refutation.

## Ссылки
`crates/stellwerk-task/src/evidence.rs:218-258` (`EvidenceStatus`, `evidence_status`, `label`); `crates/stellwerk-task/src/evidence.rs:284` (`evaluate`); `crates/stellwerk-task/src/runner.rs:337-357` (`ResumeOutcome` seam); `crates/stellwerk-task/tests/refutation.rs`; `crates/stellwerk-core/src/contract.rs:231-272` (`TaskStatus::Refuted`).
