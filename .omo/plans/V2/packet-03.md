---
id: V2/03
task_id: V2-03
milestone: V2
status: proposed
depends_on: [V2/02, 02, N1/04, N2/00b, V1/o02, V1/o03, V1/q01, V1/q02, V1/s01, V1/s02, V1/s03]
files_allowed: [crates/stellwerk-task/src/runner.rs, crates/stellwerk-task/tests/human_validation.rs, crates/stellwerk-task/tests/runner.rs]
max_files: 3
max_diff_lines: 200
acceptance: cargo test -p stellwerk-task --test human_validation --test runner
---

# V2/03 — F-03b: resume a run from the evidence status before executing

## Цель
Цель: V2-03 F-03b: resume a run from the evidence status before executing

## Шаг 1 — падающий тест
RED: тест со счётным worker — пустой ledger даёт `Proceeded` и один вызов; attested ledger без `Validation` даёт `AwaitingHumanValidation` и ноль вызовов; attested ledger + human `Validation` даёт `AlreadyHumanValidated` и ноль вызовов. До появления `run_once_resuming` тест падает.

## Что делать
add `pub enum ResumeOutcome { AlreadyHumanValidated, AwaitingHumanValidation, Proceeded(CallVerdict) }` and `pub async fn run_once_resuming(attempt: &CallAttempt, worker: &dyn Worker, store: &impl EventStore, ledger: &dyn EvidenceLedger) -> Result<ResumeOutcome, RunError>` in `crates/stellwerk-task/src/runner.rs`. Behavior: `HumanValidated` -> return `AlreadyHumanValidated` and do not call the worker; `AwaitingHumanValidation` -> return `AwaitingHumanValidation` and do not call the worker; `Unattested` -> call `run_once` and wrap `Proceeded(verdict)`. Do NOT wire `run_once_resuming` into `crates/stellwerk-cli/src/lib.rs::run_task`: `run_task` opens a fresh `unique_temp_path("evidence","db")` at lines 125-127 on every invocation, so the resume seam always sees an empty ledger and is unreachable in production. Record the live wiring as FINDING `F-2026-11-10-03`; the library seam is tested directly. Tests in `tests/human_validation.rs` and `tests/runner.rs` use a counting worker. Must NOT overload `run_once`, must NOT wrap `RunOutcome` (it is not returned by `run_once`), must NOT use `unwrap_or` to pick a status.

## Приёмка
`cargo test -p stellwerk-task --test human_validation --test runner`; a new test uses a counting worker: empty ledger -> `Proceeded` and one call; attested ledger without Validation -> `AwaitingHumanValidation` and zero calls; attested ledger plus human `Validation` -> `AlreadyHumanValidated` and zero calls; `Refuted` is checked before `ToolAttestation` once Task 11 adds it.

## QA
happy - the counting-worker test; failure - a foreign-task `Validation` must not produce `AlreadyHumanValidated`. Evidence `.omo/evidence/task-4-stellwerk-production-v2.md`.

## Запрещено
- Behavior: `HumanValidated` -> return `AlreadyHumanValidated` and do not call the worker
- `AwaitingHumanValidation` -> return `AwaitingHumanValidation` and do not call the worker
- Do NOT wire `run_once_resuming` into `crates/stellwerk-cli/src/lib.rs::run_task`: `run_task` opens a fresh `unique_temp_path("evidence","db")` at lines 125-127 on every invocation, so the resume seam always sees an empty ledger and is unreachable in production.
- Must NOT overload `run_once`, must NOT wrap `RunOutcome` (it is not returned by `run_once`), must NOT use `unwrap_or` to pick a status.

## Ссылки
`crates/stellwerk-task/src/runner.rs:337-357` (`suspension_status`, `resume_after_human_validation`); `crates/stellwerk-task/src/runner.rs:427-509` (`run_once` returns `Result<CallVerdict, RunError>`); `crates/stellwerk-task/src/evidence.rs:218-258` (`EvidenceStatus`), `:247-259` (`evidence_status`); `crates/stellwerk-cli/src/lib.rs:125-127` (fresh ledger, why live wiring is unreachable); `crates/stellwerk-task/tests/human_validation.rs` (LeakyLedger and fixture chain); `crates/stellwerk-task/tests/runner.rs`.
