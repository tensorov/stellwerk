---
id: s01
milestone: V1
status: GREEN
depends_on: [v04, i02]
files_allowed: [crates/stellwerk-task/src/runner.rs, crates/stellwerk-task/tests/suspension.rs]
overlap_waiver: C9 — N1/04, N2/00b — `runner.rs`, последовательное исполнение
acceptance: cargo test -p stellwerk-task --test suspension
---

# s01 — Автомат приостановки

## Цель
Переход `Suspended → AwaitingHumanValidation` возможен только при наличии вопроса человеку, а его отсутствие даёт отличимый отказ, потому что приостановка без вопроса — это зависание, а не ожидание решения.

## Обоснование (измерено)
- `crates/stellwerk-task/src/runner.rs:446` — здесь обрабатывается вердикт `Suspended`, и переход дальше пока не требует вопроса человеку.
- `crates/stellwerk-core/src/contract.rs:185` — здесь объявлены `Suspended` и `AwaitingHumanValidation`, между которыми нужен явный автомат.

## Шаг 1 — падающий тест
`crates/stellwerk-task/tests/suspension.rs`, тест `non_interactive_run_does_not_hang_and_does_not_allow`: утверждает, что неинтерактивный прогон не зависает и не разрешает вызов; до автомата прогон либо висит на ожидании, либо доходит до разрешения, и тест падает по таймауту или на разрешённом вызове.

## Приёмка
cargo test -p stellwerk-task --test suspension
Ожидается PASS.

## Запрещено
- нельзя вводить таймаут как разрешение.
- запрещено переходить в `AwaitingHumanValidation` без вопроса человеку.
- нельзя оставлять неинтерактивный прогон зависшим.
