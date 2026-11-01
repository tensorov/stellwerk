---
id: a02
milestone: V1
status: GREEN
depends_on: [a01, r03]
files_allowed: [crates/stellwerk-task/src/m5/gate_channel.rs, crates/stellwerk-cli/src/build_runner.rs, crates/stellwerk-cli/tests/host_tools.rs]
overlap_waiver: C9 — N2/00e, N2/00i1, N2/00h3b, N2/00h3c, N2/00h3d, N2/00j1, последовательное исполнение
acceptance: cargo test -p stellwerk-cli --test host_tools
---

# a02 — Хост исполняет только `AuthorizedCall`

## Цель
Хост применяет к workspace только `AuthorizedCall` и сверяет digest до и после записи, потому что запись по пути без сверки digest исполняет не то, что было авторизовано.

## Обоснование (измерено)
- `crates/stellwerk-cli/src/build_runner.rs:82` — здесь начинается работа с workspace, и запись обязана идти из авторизованного вызова.
- `crates/stellwerk-cli/src/build_runner.rs:355` — блок исполнения сессии — место, где результат хоста возвращается в сокет.
- `crates/stellwerk-task/src/m5/sandbox_bwrap.rs:184` — здесь заданы границы workspace, вне которых правка недопустима.

## Шаг 1 — падающий тест
`crates/stellwerk-cli/tests/host_tools.rs`, тест `write_with_mismatched_digest_is_refused`: утверждает, что запись с несовпавшим digest отвергается и файл не меняется, а успешная запись оставляет digest, равный авторизованному; до применения `AuthorizedCall` запись идёт без сверки, файл меняется при несовпадении, и тест падает на содержимом файла.

## Приёмка
cargo test -p stellwerk-cli --test host_tools
Ожидается PASS.

## Запрещено
- нельзя писать при отказе.
- запрещено позволять правку вне workspace.
- нельзя применять вызов, не являющийся `AuthorizedCall`.
