---
id: s02
milestone: V1
status: GREEN
depends_on: [s01]
files_allowed: [crates/stellwerk-task/src/runner.rs, crates/stellwerk-task/tests/human_validation.rs]
overlap_waiver: C9 — N1/04, N2/00b — `runner.rs`, последовательное исполнение
acceptance: cargo test -p stellwerk-task --test human_validation
---

# s02 — Возобновление после человеческой валидации

## Цель
`HumanValidated` возобновляет продолжение только с записью и только для своего `task_id`, потому что валидация чужой задачи возобновляет прогон, которого человек не видел.

## Обоснование (измерено)
- `crates/stellwerk-task/src/runner.rs:272` — здесь приостановка связывается с отсутствием ответа, и возобновление начинается из этой точки.
- `crates/stellwerk-task/src/evidence.rs:178` — здесь человеческая `Validation` отличается от фикстуры, и возобновление обязано опираться именно на неё.

## Шаг 1 — падающий тест
`crates/stellwerk-task/tests/human_validation.rs`, тест `resume_only_on_own_task_validation`: утверждает, что валидация чужой задачи не возобновляет прогон; до проверки `task_id` запись от другой задачи принимается, и тест падает на возобновлённом чужом прогоне.

## Приёмка
cargo test -p stellwerk-task --test human_validation
Ожидается PASS.

## Запрещено
- нельзя принимать валидацию без `task_id`.
- запрещено возобновлять по валидации чужой задачи.
- нельзя возобновлять прогон без записи.
