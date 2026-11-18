---
id: N2/00j4
milestone: N2
title: Backend предлагает инструменты сервису
depends_on: [N2/00j3a, 00g3b]
allowed_changes:
  - crates/stellwerk-task/src/m5/model_backend.rs
  - crates/stellwerk-task/tests/model_backend.rs
---

# 00j4 — Backend предлагает инструменты сервису

## Зачем

`HttpBackend` не переопределяет `turn_request` и не рендерит `tools`, поэтому модель
не получает определений функций и не может вернуть `tool_calls`. При этом `Some([])`
и `None` обязаны давать тело без ключа `tools`: пустой набор — это не «предложить
всё» и не «предложить пустое имя». Различие состояний сохраняется на проводе.

## Что делается

1. `HttpBackend` переопределяет `turn_request`.
2. `turn` делегирует ему с `tools: None` (без рекурсии, потому что `turn_request` не
   зовёт `turn`).
3. При `Some` непустом в тело добавить
   `"tools":[{"type":"function","function":{"name","description","parameters"}}]`
   в порядке `TurnRequest.tools`.
4. При `None` и `Some([])` ключа `"tools"` в теле нет.
5. Разбор ответа не менять.
6. Не добавлять `tool_choice`, `parallel_tool_calls`, стриминг.

## RED

- `the_request_offers_the_turn_tools`
- `an_empty_tool_set_sends_no_tools_key` (`Some([])`)
- `a_legacy_turn_sends_no_tools_key` (`None`)
- `tools_render_as_function_definitions`

## Приёмка

```
cargo test -p stellwerk-task --test model_backend
cargo test -p stellwerk-task --test model_transport_wire
cargo clippy -p stellwerk-task --all-targets --no-deps -- -D warnings
cargo fmt -p stellwerk-task -- --check
```

Ожидание: четыре теста зелёные; wire-тесты не регрессировали; clippy/fmt чисто.

## Не делается

Не добавляются `tool_choice`, `parallel_tool_calls`, стриминг. Разбор ответа не
меняется. Никаких новых зависимостей, `#[allow]`/`#[ignore]`.

## Бюджет

Не больше 3 изменённых файлов. C2: src ≤ 200 логически добавленных строк,
tests ≤ 250 добавленных строк. Новые зависимости не добавляются. `#[allow(...)]`
и `#[ignore]` запрещены.
