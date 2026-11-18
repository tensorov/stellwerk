---
id: N2/00j5
milestone: N2
title: Результат инструмента возвращается в разговор
depends_on: [N2/00j4, 00j3, 00j4]
allowed_changes:
  - crates/stellwerk-task/src/m5/model_transport.rs
  - crates/stellwerk-task/src/m5/model_backend.rs
  - crates/stellwerk-task/tests/model_backend.rs
---

# 00j5 — Результат инструмента возвращается в разговор

## Зачем

Транскрипт теряет форму разговора: в нём нет assistant-сообщения с `tool_calls` и
`tool`-сообщения с `tool_call_id`, поэтому второй ход не может вернуть результат
инструмента к тому вызову, который его породил. Без явных вариантов сообщений и
валидации по роли разговор с инструментами невоспроизводим, а неизвестный ключ
молча принимается.

## Что делается

1. `TurnMessage` — internally tagged enum (`role`): `system{text}`, `user{text}`,
   `assistant{text: Option<String>, #[serde(default)] tool_calls: Vec<AssistantCall>}`,
   `tool{tool_call_id, name, text, is_error}`.
2. `AssistantCall{id, name, arguments: Value}`.
3. **Не** ставить `deny_unknown_fields` на enum — `parse` проверяет набор ключей
   сырого объекта по роли (неизвестный ключ ⇒ `Schema`), затем десериализует.
4. Валидация: assistant без текста и вызовов, пустой `ToolCall.id`, `arguments` не
   объект, пустой `tool_call_id` — `Schema`.
5. Рендер: system/user как есть; assistant — `content: text|null`, `tool_calls`
   (ключ опускается при пустом), `arguments` **строкой**; tool —
   `{"role":"tool","tool_call_id","content"}`, при `is_error` контент в
   `<tool_error>…</tool_error>`.
6. `TurnMessage: Clone`.
7. Обновить `user()` в `tests/model_backend.rs:26-31`.
8. Не реплеить `thinking`, не передавать картинки (пометка
   `[non-text content omitted: N]`).

## RED

- `a_tool_result_keeps_the_call_id_it_answers`
- `an_assistant_turn_carries_the_calls_it_made`
- `a_system_message_stays_a_system_message`
- `tool_results_render_as_openai_tool_messages`
- `an_assistant_tool_call_renders_arguments_as_a_string`
- `a_tool_message_without_a_call_id_is_refused`
- `an_assistant_with_neither_text_nor_calls_is_refused`
- `an_unknown_message_key_is_refused`
- `tool_calls_default_to_empty`

## Приёмка

```
cargo test -p stellwerk-task --test model_backend
cargo test -p stellwerk-task --test model_transport_wire
cargo test -p stellwerk-cli --test planner_host
cargo clippy -p stellwerk-task --all-targets --no-deps -- -D warnings
cargo fmt -p stellwerk-task -- --check
```

Ожидание: весь набор message-shapes зелёный; wire и planner_host не
регрессировали; clippy/fmt чисто.

## Не делается

Не реплеится `thinking`, картинки не передаются (только пометка
`[non-text content omitted: N]`). Не меняются `permission.rs`, `gate.rs`,
`contract.rs`, классификация `BUILD_TOOLS`. Никаких новых зависимостей,
`#[allow]`/`#[ignore]`.

## Бюджет

Не больше 3 изменённых файлов. C2: src ≤ 200 логически добавленных строк,
tests ≤ 250 добавленных строк. Новые зависимости не добавляются. `#[allow(...)]`
и `#[ignore]` запрещены.
