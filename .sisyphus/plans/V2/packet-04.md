---
id: V2/04
task_id: V2-04
milestone: V2
status: proposed
depends_on: [N2/00h2b, N2/00h3a, N2/00h3d, N2/00h3e, V1/r01]
files_allowed: [crates/stellwerk-cli/src/plan.rs, crates/stellwerk-cli/tests/plan.rs]
max_files: 3
max_diff_lines: 200
acceptance: cargo test -p stellwerk-cli --test plan
---

# V2/04 — F-04: show the build decision in the plan TUI

## Цель
Цель: V2-04 F-04: show the build decision in the plan TUI

## Шаг 1 — падающий тест
RED: тест `build_status_names_every_decision` падает до появления `build_status`; `Deny` не должен рендериться пустым ответом.

## Что делать
add `pub fn build_status(outcome: &BuildOutcome) -> String` in `crates/stellwerk-cli/src/plan.rs` that includes `verdict: <decision>` plus `model_turns`, `gate_summary`, and `answer_text`; `PlanUi::drain_build` uses it. Reuse the existing `verdict_name` from `crates/stellwerk-cli/src/lib.rs`; do not add a second label function. Must NOT drop `Deny`/`Suspended`/`Ask`. Must NOT print an empty agent answer as success.

## Приёмка
`cargo test -p stellwerk-cli --test plan`; a new test calls `build_status` with `Allow`, `Deny { reason }`, `Suspended`, `Ask` and asserts each label appears; a `Deny` case must not render as an empty answer.

## QA
happy - `cargo test -p stellwerk-cli --test plan build_status_names_every_decision`; failure - a `Deny` outcome whose `answer_text` is empty still shows `Deny reason=...`. Evidence `.omo/evidence/task-5-stellwerk-production-v2.md`.

## Запрещено
- do not add a second label function.
- Must NOT drop `Deny`/`Suspended`/`Ask`.
- Must NOT print an empty agent answer as success.

## Ссылки
`crates/stellwerk-cli/src/plan.rs:1040-1065` (`drain_build`); `crates/stellwerk-cli/src/build_runner.rs:650-680` (`BuildOutcome.decision`); `crates/stellwerk-cli/src/lib.rs:283` (`verdict_name`); `crates/stellwerk-cli/tests/plan.rs`.
