---
id: r01
milestone: V1
status: GREEN
depends_on: [01]
files_allowed: [crates/stellwerk-cli/src/lib.rs, crates/stellwerk-cli/src/main.rs, crates/stellwerk-cli/src/plan.rs, crates/stellwerk-cli/src/build_runner.rs, crates/stellwerk-cli/tests/run.rs]
limit_waiver: 5 файлов — один путь исполнения, run_build больше не зовёт worker.exec напрямую
overlap_waiver: C9 — N1/01, N1/10, N2/00h1, N2/00h2a, N2/00h2b, N2/00h3b, TUI/02, TUI/03, последовательное исполнение
acceptance: cargo test -p stellwerk-cli --test run
---

# r01 — Один путь исполнения вместо plan-Build и run

## Цель
Plan-Build и run сходятся в одну авторизующую функцию, и оба входа проходят один и тот же `authorize`, потому что два пути исполнения позволяют обойти гейт, не меняя ни одного теста гейта.

## Обоснование (измерено)
- `crates/stellwerk-cli/src/lib.rs:53` — run-путь начинается здесь и сейчас живёт отдельно от Build-пути.
- `crates/stellwerk-cli/src/main.rs:54` — диспетчеризация двух подкоманд идёт из `main`, поэтому сведение путей начинается тут.
- `crates/stellwerk-cli/src/build_runner.rs:455` — `run_build` вызывает `worker.exec` напрямую, минуя общий `authorize`.

## Шаг 1 — падающий тест
`crates/stellwerk-cli/tests/run.rs`, тест `both_entrypoints_share_one_authorization`: утверждает, что вход через plan-Build и вход через run логируют один и тот же вызов `authorize`; до сведения путей Build-путь исполняет напрямую, поэтому общего `authorize` в журнале нет и тест падает на сравнении журналов.

## Приёмка
cargo test -p stellwerk-cli --test run
Ожидается PASS.

## Запрещено
- нельзя оставлять прямой `worker.exec` в обход авторизации.
- запрещено заводить второй модуль-дубликат вместо сведения путей.
- нельзя менять поведение гейта ради прохождения теста.
