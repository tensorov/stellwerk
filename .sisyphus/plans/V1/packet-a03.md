---
id: a03
milestone: V1
status: GREEN
depends_on: [a01]
files_allowed: [crates/stellwerk-task/src/orchestrator.rs, crates/stellwerk-task/tests/authorization.rs]
overlap_waiver: C9 — N1/03, N1/05, N1/06, N1/09, N2/00c, N2/00f — `orchestrator.rs`, `tests/authorization.rs`, последовательное исполнение
acceptance: cargo test -p stellwerk-task --test authorization
---

# a03 — Авторитет классификации риска принадлежит хосту

## Цель
Риск инструмента берётся только из `StaticRegistry`, а неизвестный инструмент даёт `Unclassified`, отличимый от отказа, потому что риск, названный вызывающим, — это утверждение агента о самом себе.

## Обоснование (измерено)
- `crates/stellwerk-task/src/orchestrator.rs:70` — здесь `risk_for` принимает риск от вызывающего, вместо того чтобы читать запись реестра.
- `crates/stellwerk-task/src/orchestrator.rs:121` — решение по уровню принимается в этом же блоке, поэтому источник риска меняется здесь.

## Шаг 1 — падающий тест
`crates/stellwerk-task/tests/authorization.rs`, тест `host_classifies_risk_not_caller`: утверждает, что агент, назвавший `bash` как `R0`, получает решение `R5`, а неизвестный инструмент даёт `Unclassified`, отличимый от `Deny`; до правки `risk_for` доверяет названному риску, поэтому `bash` проходит как `R0` и тест падает на первом утверждении.

## Приёмка
cargo test -p stellwerk-task --test authorization
Ожидается PASS.

## Запрещено
- нельзя брать риск из аргумента вызывающего.
- запрещено добавлять дефолт риска для неизвестного инструмента.
- нельзя сливать `Unclassified` с `Deny` в один статус.
