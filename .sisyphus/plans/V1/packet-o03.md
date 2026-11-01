---
id: o03
milestone: V1
status: GREEN
depends_on: [o02]
files_allowed: [crates/stellwerk-task/src/evidence.rs, crates/stellwerk-task/tests/human_validation.rs]
overlap_waiver: C9 — N1/07 — `evidence.rs`, последовательное исполнение
acceptance: cargo test -p stellwerk-task --test human_validation
---

# o03 — Evidence человеческой валидации

## Цель
Переход `AwaitingHumanValidation → HumanValidated` требует явной человеческой записи с `task_id`, потому что молчание, зачтённое как валидация, неотличимо от решения человека.

## Обоснование (измерено)
- `crates/stellwerk-core/src/contract.rs:185` — здесь объявлены состояния, между которыми переход обязан требовать запись.
- `crates/stellwerk-task/src/evidence.rs:111` — здесь принимается `Validation`, и её допустимость должна зависеть от явного человеческого решения.

## Шаг 1 — падающий тест
`crates/stellwerk-task/tests/human_validation.rs`, тест `transition_requires_explicit_record`: утверждает, что без записи переход запрещён, а с записью — разрешён; до проверки записи переход проходит при пустой цепочке evidence, и тест падает на разрешённом переходе без записи.

## Приёмка
cargo test -p stellwerk-task --test human_validation
Ожидается PASS.

## Запрещено
- нельзя считать молчание валидацией.
- запрещено принимать `Validation` без `task_id`.
- нельзя разрешать переход в `HumanValidated` без записи.
