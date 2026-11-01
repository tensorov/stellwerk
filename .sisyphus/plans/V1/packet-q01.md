---
id: q01
milestone: V1
status: GREEN
depends_on: [s03]
files_allowed: [crates/stellwerk-task/src/runner.rs, crates/stellwerk-task/tests/quarantine.rs]
overlap_waiver: C9 — N1/04, N2/00b — `runner.rs`, последовательное исполнение
acceptance: cargo test -p stellwerk-task --test quarantine
---

# q01 — Квартир отказанных прогонов

## Цель
Отказанный прогон попадает в карантин с причиной и не смешивается с успешными кандидатами, потому что отказанный артефакт без карантина либо теряется, либо притворяется успешным.

## Обоснование (измерено)
- `crates/stellwerk-task/src/runner.rs:446` — здесь отказ отделяется от приостановки, и отсюда отказанный прогон должен уходить в карантин.
- `crates/stellwerk-cli/src/build_runner.rs:82` — сборка кандидата начинается в workspace, поэтому отказанный прогон обязан не доходить до неё.

## Шаг 1 — падающий тест
`crates/stellwerk-task/tests/quarantine.rs`, тест `refused_run_is_quarantined`: утверждает, что отказ даёт запись в карантине и не даёт кандидата; до появления карантина отказанный прогон оставляет только статус, и тест падает на отсутствии записи.

## Приёмка
cargo test -p stellwerk-task --test quarantine
Ожидается PASS.

## Запрещено
- нельзя удалять отказанные артефакты молча.
- запрещено смешивать карантин с успешными кандидатами.
- нельзя заводить карантинную запись без причины.
