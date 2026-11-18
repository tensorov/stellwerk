---
id: o04
milestone: V1
status: GREEN
depends_on: [o03]
files_allowed: [crates/stellwerk-task/src/evidence.rs, crates/stellwerk-task/tests/refutation.rs]
overlap_waiver: C9 — N1/07 — `evidence.rs`, последовательное исполнение
acceptance: cargo test -p stellwerk-task --test refutation
---

# o04 — Evidence опровержения

## Цель
`Refuted` принимается только с названным основанием и ссылкой на опровергающие данные, потому что опровержение без основания неотличимо от отказа без причины.

## Обоснование (измерено)
- `crates/stellwerk-core/src/contract.rs:185` — здесь объявлен статус `Refuted`, и переход в него обязан нести основание.
- `crates/stellwerk-task/src/evidence.rs:196` — здесь проверяется наличие звеньев цепочки, к которой добавляется проверка основания опровержения.

## Шаг 1 — падающий тест
`crates/stellwerk-task/tests/refutation.rs`, тест `refuted_requires_stated_basis`: утверждает, что `Refuted` без основания отвергается, а с названным основанием принимается; до проверки основания `Refuted` принимается с пустой причиной, и тест падает на принятом пустом основании.

## Приёмка
cargo test -p stellwerk-task --test refutation
Ожидается PASS.

## Запрещено
- нельзя превращать `Refuted` в `Failed`.
- запрещено принимать `Refuted` без основания.
- нельзя терять ссылку на опровергающие данные.
