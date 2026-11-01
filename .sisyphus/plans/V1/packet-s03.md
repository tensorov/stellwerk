---
id: s03
milestone: V1
status: GREEN
depends_on: [s02]
files_allowed: [crates/stellwerk-task/src/runner.rs, crates/stellwerk-task/tests/refutation.rs]
overlap_waiver: C9 — N1/04, N2/00b — `runner.rs`, последовательное исполнение
acceptance: cargo test -p stellwerk-task --test refutation
---

# s03 — Опровержение останавливает цепочку

## Цель
`Refuted` запрещает дальнейшие переходы и требует основания, потому что «опровергнуто, но завершено» делает опровержение декоративным.

## Обоснование (измерено)
- `crates/stellwerk-task/src/runner.rs:487` — здесь первый не-разрешающий вердикт останавливает прогон, и `Refuted` обязан останавливать его так же необратимо.
- `crates/stellwerk-core/src/contract.rs:185` — здесь объявлен `Refuted`, переходы из которого нужно закрыть.

## Шаг 1 — падающий тест
`crates/stellwerk-task/tests/refutation.rs`, тест `done_after_refuted_is_rejected`: утверждает, что после `Refuted` переход в `Done` отвергается; до запрета переходов прогон доходит до `Done`, и тест падает на принятом завершении после опровержения.

## Приёмка
cargo test -p stellwerk-task --test refutation
Ожидается PASS.

## Запрещено
- нельзя позволять «опровергнуто, но завершено».
- запрещено разрешать переходы после `Refuted`.
- нельзя терять основание опровержения.
