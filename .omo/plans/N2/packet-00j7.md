---
id: N2/00j7
milestone: N2
title: Ход без tools отвергается
depends_on: [N2/00j6, 00h1, 00j3, 00j5]
allowed_changes:
  - crates/stellwerk-task/src/m5/model_transport.rs
  - crates/stellwerk-task/tests/model_transport_wire.rs
  - crates/stellwerk-cli/tests/host_model.rs
---

# 00j7 — Ход без tools отвергается

## Зачем

Последний шаг делает `tools` обязательным: молчаливое отсутствие ключа после 00j6
означало бы legacy-клиента с неизвестным набором, и «никто не ответил» снова стало
бы разрешением. Отсутствие `tools` — `schema`; `[]` остаётся сознательным
«инструментов нет». Так состояние «неизвестно» исчезает из протокола, а не
превращается в значение по умолчанию.

## Что делается

1. `parse` требует ключ `tools`; `None` ⇒ `ErrorCode::Schema` с сообщением
   «turn has no `tools` field».
2. `Some([])` остаётся допустимым.
3. `a_legacy_turn_is_forwarded_without_offer_validation` заменяется на
   `a_turn_without_tools_is_refused`; добавить `an_empty_tool_set_is_an_explicit_choice`.
4. `TURN` получает **валидный `ToolDef` для bash**
   (`{"name":"bash","description":"shell","parameters":{"type":"object"}}`), потому
   что roundtrip-тест ждёт `tool_use`; для отказа — отдельный `NO_TOOLS` payload без
   ключа, для пустого — `EMPTY_TOOLS` с `"tools":[]`.
5. В `tests/model_transport_wire.rs` у `Scripted` и `Failing` добавить
   `turn_request`: оба делегируют в `turn` (иначе `TURN` с `Some([bash])` падает на
   умолчании `BackendError("backend does not accept tools")`, и
   `a_turn_roundtrip...`/`a_backend_error...` не доходят до сценария).
6. Добавить `"tools":[]` в at-limit и второй ход
   (`tests/model_transport_wire.rs:177,216-217`) и в
   `crates/stellwerk-cli/tests/host_model.rs` (`exchange`, `:49-58`).
7. Route-тест (`:129-140`) не трогать: неизвестное поле проверяется до `tools`,
   поэтому `UnexpectedField` остаётся.
8. Проверка лишнего поля идёт **до** требования `tools`.
9. Схему сообщений не менять.

## RED

- `a_turn_without_tools_is_refused` (`NO_TOOLS`)
- `an_empty_tool_set_is_an_explicit_choice` (`EMPTY_TOOLS` + text backend → Done
  `stop`)
- `an_unknown_field_is_still_refused_first`
- `a_turn_roundtrip_streams_text_tool_and_done` по-прежнему `tool_use`
- `a_backend_error_is_an_error_frame_not_done` по-прежнему `BackendError` (через
  явный `Failing::turn_request`)
- `a_legacy_turn_is_forwarded_without_offer_validation` заменяется на
  `a_turn_without_tools_is_refused`.

## Приёмка

```
cargo test -p stellwerk-task --test model_transport_wire
cargo test -p stellwerk-task --test pi_transport
cargo test -p stellwerk-cli --test host_model
cargo clippy -p stellwerk-task --all-targets --no-deps -- -D warnings
cargo fmt -p stellwerk-task -- --check
```

Ожидание: три набора зелёные; `a_turn_roundtrip_streams_text_tool_and_done` даёт
`tool_use`; `a_backend_error_is_an_error_frame_not_done` даёт `BackendError`;
clippy/fmt чисто.

## Не делается

Схема сообщений не меняется. Route-тест не трогается: `UnexpectedField` остаётся.
Отсутствие `tools` не превращается в пустой набор по умолчанию. Никаких новых
зависимостей, `#[allow]`/`#[ignore]`.

## Бюджет

Не больше 3 изменённых файлов. C2: src ≤ 200 логически добавленных строк,
tests ≤ 250 добавленных строк. Новые зависимости не добавляются. `#[allow(...)]`
и `#[ignore]` запрещены.
