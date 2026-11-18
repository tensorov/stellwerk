---
id: o02
milestone: V1
status: GREEN
depends_on: [o01]
files_allowed: [crates/stellwerk-task/src/runner.rs, crates/stellwerk-task/tests/runner.rs]
overlap_waiver: C9 — N1/04, N2/00b — `runner.rs`, последовательное исполнение
acceptance: cargo test -p stellwerk-task --test runner
---

# o02 — `Suspended` как отдельное состояние с evidence

## Цель
`runner` различает «запрещено политикой» и «никто не ответил», и `Suspended` пишет evidence с причиной, потому что слияние приостановки с отказом делает отсутствие ответа неотличимым от сознательного запрета.

## Обоснование (измерено)
- `crates/stellwerk-task/src/runner.rs:52` — здесь начинается прогон, где вердикт превращается в статус.
- `crates/stellwerk-task/src/runner.rs:296` — здесь обрабатывается не-разрешающий вердикт, и приостановка пока не отделена от отказа.
- `crates/stellwerk-gateway/src/gate.rs:33` — вердикт гейта различает отказ и вопрос, поэтому различение обязано дожить до статуса.

## Шаг 1 — падающий тест
`crates/stellwerk-task/tests/runner.rs`, тест `suspended_and_deny_are_distinct`: утверждает, что неинтерактивный `Ask` даёт `Suspended`, а `Deny` даёт `Deny`, и статусы различимы; до разделения оба случая дают `Deny`, и тест падает на равенстве статусов.

## Приёмка
cargo test -p stellwerk-task --test runner
Ожидается PASS.

## Запрещено
- нельзя сливать `Suspended` в `Deny`.
- запрещено оставлять неинтерактивный `Ask` без evidence.
- нельзя выдавать молчание за разрешение.
