---
id: V2/02
task_id: V2-02
milestone: V2
status: proposed
depends_on: [N1/04, N2/00b, V1/o02, V1/q01, V1/q02, V1/s01, V1/s02, V1/s03]
files_allowed: [crates/stellwerk-task/src/lifecycle.rs, crates/stellwerk-task/src/runner.rs, crates/stellwerk-task/tests/lifecycle.rs]
max_files: 3
max_diff_lines: 200
acceptance: cargo test -p stellwerk-task --test lifecycle --test suspension
---

# V2/02 — F-03a: wire the suspension seam without overloading next_status

## Цель
Цель: V2-02 F-03a: wire the suspension seam without overloading next_status

## Шаг 1 — падающий тест
RED: тесты `ask_from_running_reaches_awaiting_human_validation` и `suspended_from_running_stays_suspended` падают до правки; проверка — `cargo test -p stellwerk-task --test lifecycle --test suspension`.

## Что делать
keep `approval::next_status` exactly as it is (risk-aware `Suspended`/`Failed`; it never returns `Running`). Keep the one-arg `runner::suspension_status(verdict: &CallVerdict) -> Option<TaskStatus>` unchanged for `tests/suspension.rs`; add `pub fn suspension_status_from(from: TaskStatus, verdict: &CallVerdict) -> Option<TaskStatus>`: `Ask` returns `AwaitingHumanValidation` when `from.can_transition_to(AwaitingHumanValidation)`, otherwise `Suspended` when `from.can_transition_to(Suspended)`, otherwise `None`; `Suspended` returns `Suspended` when legal; `Allow`/`Deny` return `None`. Add `pub fn apply_suspension(store: &impl TaskStore, id: &str, verdict: &CallVerdict) -> Result<Option<TaskStatus>, LifecycleError>` in `lifecycle.rs` that reads `status_of`, then loops `suspension_status_from(current, verdict)` + `advance(store, id, next)` until `None`, returning the last landed status. For `Ask` from `Running` this is two legal steps (`Running -> Suspended -> AwaitingHumanValidation`); for `Suspended` it is one. Tests in `tests/lifecycle.rs` assert both sequences. The live orchestrator call site does not exist (the CLI `run_task` has no live task-status lifecycle/row; its `SqliteStore` is an event store, not a `TaskStore` status row), so record it as FINDING `F-2026-11-10-02`; do not claim the orchestrator is wired. Must NOT change `next_status`'s meaning, must NOT write `Running -> AwaitingHumanValidation` directly, must NOT use `unwrap_or(Suspended)`.

## Приёмка
`cargo test -p stellwerk-task --test lifecycle --test suspension`; `cargo test -p stellwerk-cli --test vertical_v4`; `ask_from_running_reaches_awaiting_human_validation` asserts `Running -> Suspended -> AwaitingHumanValidation`; `suspended_from_running_stays_suspended` asserts `Running -> Suspended`; `next_status(Ask, R3)` still returns `Transition(Suspended)`.

## QA
happy - the lifecycle/suspension tests; failure - a direct `Running -> AwaitingHumanValidation` attempt must return `IllegalTransition`. Evidence `.omo/evidence/task-3-stellwerk-production-v2.md`.

## Запрещено
- do not claim the orchestrator is wired.
- Must NOT change `next_status`'s meaning, must NOT write `Running -> AwaitingHumanValidation` directly, must NOT use `unwrap_or(Suspended)`.

## Ссылки
`crates/stellwerk-task/src/approval.rs:104-120` (`next_status`); `crates/stellwerk-task/src/runner.rs:337-343` (`suspension_status`); `crates/stellwerk-task/src/lifecycle.rs:66-107` (`advance`); `crates/stellwerk-core/src/contract.rs:231-258` (`can_transition_to`); `crates/stellwerk-task/tests/lifecycle.rs:94-115`; `crates/stellwerk-task/tests/suspension.rs:125-155`; `crates/stellwerk-cli/tests/vertical_v4.rs:146` (must stay green because `next_status` is unchanged).
