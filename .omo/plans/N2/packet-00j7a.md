---
id: N2/00j7a
milestone: N2
title: Тестовый wire-хелпер model_backend объявляет пустой набор
depends_on: [N2/00j7, 00j5]
allowed_changes:
  - crates/stellwerk-task/tests/model_backend.rs
---

# 00j7a — Тестовый wire-хелпер model_backend объявляет пустой набор

## Зачем

00j7 сделал `tools` обязательным в кадре `turn`: отсутствие ключа теперь
`ErrorCode::Schema` («turn has no `tools` field»). `tests/model_backend.rs` собирает
wire-кадры через собственный хелпер `turn(messages)` (`:494-495`) без `tools`,
поэтому `tool_calls_default_to_empty` перестал доходить до backend'а. Файл не входит
в `00j7.allowed_changes`, а расширять пакет до четырёх файлов нельзя. Отдельный
test-only follow-up сохраняет границу пакета и возвращает полный `model_backend`
в зелёное состояние.

## Что делается

1. В `turn(messages)` (`crates/stellwerk-task/tests/model_backend.rs:494-495`)
   формировать кадр с явным пустым набором:
   `format!("{{\"kind\":\"turn\",\"tools\":[],\"messages\":{messages}}}\n")`.
2. Оставить `a_legacy_turn_sends_no_tools_key` (`:281`) без изменений: он проверяет
   `TurnRequest { tools: None }` на уровне backend, а не wire-хелпер.
3. Не менять `tool_calls_default_to_empty` и его ожидания: тест должен пройти
   без ослабления.

## RED

До фикса:

```
cargo test -p stellwerk-task --test model_backend
...
test tool_calls_default_to_empty ... FAILED
  разговор дошёл до backend'а: [Error { code: Schema,
    message: "turn has no `tools` field" }]
test result: FAILED. 17 passed; 1 failed
```

## Приёмка

```
cargo test -p stellwerk-task --test model_backend
cargo test -p stellwerk-task
cargo clippy -p stellwerk-task --all-targets --no-deps -- -D warnings
rustfmt --edition 2021 --check --color never \
  crates/stellwerk-task/tests/model_backend.rs
```

Ожидание: `model_backend` 18/18; весь `stellwerk-task` зелёный; clippy/fmt чисто.
C2: src 0, tests 1, один файл.

## Не делается

Не меняется продовый код, `model_transport.rs`, `TurnRequest`/`TurnMessage`,
`model_transport_wire.rs`. Тест не ослабляется, `#[ignore]`/`#[allow]` не
добавляются. Новые зависимости не добавляются.

## Бюджет

Один файл, C2 tests ≤ 250. `max_diff_lines` не меняется. Коммит делает
оркестратор после проверки.
