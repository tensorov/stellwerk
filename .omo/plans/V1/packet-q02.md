---
id: q02
milestone: V1
status: GREEN
depends_on: [q01]
files_allowed: [crates/stellwerk-task/src/runner.rs, crates/stellwerk-task/tests/quarantine.rs]
overlap_waiver: C9 — N1/04, N2/00b — `runner.rs`, последовательное исполнение
acceptance: cargo test -p stellwerk-task --test quarantine
---

# q02 — Срок хранения карантина

## Цель
Карантин имеет явный срок, и запись старше срока помечается истёкшей, но не исчезает без явного решения, потому что удаление по таймеру уничтожает доказательство отказа.

## Обоснование (измерено)
- `crates/stellwerk-task/src/runner.rs:487` — здесь прогон останавливается на первом не-разрешающем вердикте, и срок хранения относится к записи, созданной в этой точке.

## Шаг 1 — падающий тест
`crates/stellwerk-task/tests/quarantine.rs`, тест `expired_entry_is_marked_not_deleted`: утверждает, что запись старше срока помечается истёкшей и остаётся на месте; до явного срока запись либо не имеет срока, либо чистится, и тест падает на пропавшей записи.

## Приёмка
cargo test -p stellwerk-task --test quarantine
Ожидается PASS.

## Запрещено
- нельзя удалять карантинные записи по таймеру.
- запрещено чистить карантин «по умолчанию».
- нельзя терять истёкшую запись без явного решения.
