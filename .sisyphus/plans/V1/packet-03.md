---
id: 03
milestone: V1
status: GREEN
depends_on: [02]
files_allowed: [crates/stellwerk-core/src/contract.rs, docs/adr/0022-task-status-human-and-refutation.md, crates/stellwerk-core/tests/task_lifecycle.rs]
overlap_waiver: C9 — M1/05, M1/06 — `contract.rs`, последовательное исполнение
acceptance: cargo test -p stellwerk-core --test task_lifecycle
---

# 03 — contract-change: состояния человека и опровержения

## Цель
`AwaitingHumanValidation`, `HumanValidated`, `Refuted` и `Cancelled` входят в `TaskStatus` явными вариантами, а `EvidenceEntry` получает типизированные bindings, потому что состояние, выведенное из порядка evidence, неотличимо от его отсутствия.

## Обоснование (измерено)
- `crates/stellwerk-core/src/contract.rs:185` — здесь объявлен `TaskStatus`, и новых вариантов в нём нет.
- `crates/stellwerk-core/src/contract.rs:449` — здесь объявлен `EvidenceEntry`, куда входят `recipe_digest`, `policy_digest`, `model_fingerprint` и `commit_sha`.
- `crates/stellwerk-core/src/contract.rs:437` — рядом лежит форма evidence, которую новые bindings обязаны не сломать.

## Шаг 1 — падающий тест
`crates/stellwerk-core/tests/task_lifecycle.rs`, тест `human_validation_and_refutation_are_explicit_states`: требует переходов `Suspended -> AwaitingHumanValidation -> HumanValidated` и `EvidenceSubmitted -> Refuted`; до расширения `TaskStatus` этих вариантов нет, поэтому тест не компилируется на отсутствующих вариантах, а после их появления падает на запрещённом переходе `Running -> HumanValidated`.

## Приёмка
cargo test -p stellwerk-core --test task_lifecycle
Ожидается PASS.

## Запрещено
- нельзя выводить новые состояния из порядка evidence вместо явных вариантов.
- запрещено менять lint-политику или добавлять `#[allow]` под расширение контракта.
- нельзя трогать файлы вне пакета: контракт меняется только здесь и только с ADR.
