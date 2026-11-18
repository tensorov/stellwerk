---
id: r03
milestone: V1
status: GREEN
depends_on: [r01, r02, a01, a03]
files_allowed: [crates/stellwerk-task/pi/stellwerk_transport.ts, crates/stellwerk-task/src/orchestrator.rs, crates/stellwerk-task/tests/pi_transport.rs]
overlap_waiver: C9 — N1/03, N1/05, N1/06, N1/09, N2/00c, N2/00f, N2/00i1, N2/00j6, последовательное исполнение
acceptance: cargo test -p stellwerk-task --test pi_transport
---

# r03 — Pi получает extension-инструменты вместо встроенных

## Цель
`read`/`edit`/`write` регистрируются через `pi.registerTool`, и `execute` шлёт полный типизированный вызов в `gate.sock`, потому что эффект, исполненный внутри Pi, хост не видит и не может сверить с авторизацией.

## Обоснование (измерено)
- `crates/stellwerk-task/pi/stellwerk_transport.ts:259` — здесь транспорт шлёт вызов в сокет, и это место расширяется до `execute`-инструментов.
- `crates/stellwerk-task/src/orchestrator.rs:243` — `pi_transport_args` пока передаёт встроенные имена, поэтому путь «Pi пишет сам» остаётся живым.

## Шаг 1 — падающий тест
`crates/stellwerk-task/tests/pi_transport.rs`, тест `tool_call_goes_to_socket_with_args`: утверждает, что вызов инструмента уходит в сокет с аргументами, а не исполняется внутри Pi; до регистрации extension-инструментов вызов обрабатывается встроенным инструментом, в сокет ничего не приходит, и тест падает на пустом журнале сокета.

## Приёмка
cargo test -p stellwerk-task --test pi_transport
Ожидается PASS.

## Запрещено
- нельзя оставлять путь, где Pi пишет файл сам.
- запрещено передавать встроенные имена вместо extension-инструментов.
- нельзя снимать `pi.on("tool_call")` как второй блокирующий пояс.
