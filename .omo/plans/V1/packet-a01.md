---
id: a01
milestone: V1
status: GREEN
depends_on: [03]
files_allowed: [crates/stellwerk-gateway/src/gate.rs, crates/stellwerk-task/src/m5/gate_channel.rs, crates/stellwerk-gateway/tests/gate.rs]
overlap_waiver: C9 — N2/00e, N2/00i1 — `gate_channel.rs`, последовательное исполнение
acceptance: cargo test -p stellwerk-gateway --test gate
---

# a01 — Типизированный `RequestedCall` с digest содержимого

## Цель
Запрос гейта несёт полный аргумент и `content_digest`, а `AuthorizedCall` побайтово связан с этим digest, потому что решение по паре `(tool, path)` без содержимого разрешает подменить файл после авторизации.

## Обоснование (измерено)
- `crates/stellwerk-gateway/src/gate.rs:16` — здесь начинается тип запроса гейта, и digest содержимого в нём отсутствует.
- `crates/stellwerk-task/src/m5/gate_channel.rs:26` — `GateRequest` передаётся по каналу и пока не несёт ни аргумента, ни digest.
- `crates/stellwerk-task/src/m5/gate_channel.rs:103` — обработка запроса в канале — место, где digest обязан проверяться до выдачи `AuthorizedCall`.

## Шаг 1 — падающий тест
`crates/stellwerk-gateway/tests/gate.rs`, тест `same_tool_path_different_content_gets_different_decision`: утверждает, что два вызова с одинаковым `(tool, path)`, но разным содержимым дают разные решения и разные `scope`; до появления digest решения совпадают, поэтому тест падает на равенстве вердиктов.

## Приёмка
cargo test -p stellwerk-gateway --test gate
Ожидается PASS.

## Запрещено
- нельзя принимать `path` без digest для пишущего инструмента.
- запрещено выводить digest из текста модели.
- нельзя разрешать конструировать `AuthorizedCall` вне гейта.
