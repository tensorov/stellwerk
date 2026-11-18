---
id: r04
milestone: V1
status: GREEN
depends_on: [r03]
files_allowed: [crates/stellwerk-cli/src/lib.rs, crates/stellwerk-cli/src/tui.rs, crates/stellwerk-cli/tests/run.rs]
overlap_waiver: C9 — N1/01, N2/00h1, N2/00h2a, N2/00h3b, TUI/02, TUI/03, последовательное исполнение
acceptance: cargo test -p stellwerk-cli --test run
---

# r04 — Убрать fixture-путь из `run` и TUI

## Цель
`stellwerk run` исполняет Pi по профилю `pi`, а при его отсутствии даёт названный `MissingEngine`, потому что тихий переход на фикстуру выдаёт прогон без движка за настоящий.

## Обоснование (измерено)
- `crates/stellwerk-cli/src/lib.rs:59` — здесь ищется и копируется `pi-fixture.sh` в продакшн-пути `run`.
- `crates/stellwerk-cli/src/lib.rs:150` — здесь путь исполнения выбирает движок, и фикстура попадает в тот же выбор.
- `crates/stellwerk-cli/src/tui.rs:143` — TUI повторяет поиск фикстуры, поэтому обход обязан закрыться в обоих местах.

## Шаг 1 — падающий тест
`crates/stellwerk-cli/tests/run.rs`, тест `missing_engine_is_named_refusal`: утверждает, что отсутствие Pi даёт `MissingEngine`, а его наличие запускает Pi, а не `sh`; до удаления fixture-пути прогон без движка завершается успешно через `pi-fixture.sh`, и тест падает на отсутствии названного отказа.

## Приёмка
cargo test -p stellwerk-cli --test run
Ожидается PASS.

## Запрещено
- нельзя оставлять `pi-fixture.sh` в продакшн-путях.
- запрещено тихо переходить на фикстуру вместо названного отказа.
- нельзя менять пример-фикстуру, используемую тестами.
