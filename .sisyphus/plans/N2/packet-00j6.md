---
id: N2/00j6
milestone: N2
title: Pi перестаёт быть текстовым реле
depends_on: [N2/00j5, 00f, 00i1, 00j1]
allowed_changes:
  - crates/stellwerk-task/pi/stellwerk_transport.ts
  - crates/stellwerk-task/tests/pi_transport.rs
  - crates/stellwerk-cli/tests/build_runner.rs
---

# 00j6 — Pi перестаёт быть текстовым реле

## Зачем

TS-адаптер Pi сегодня — текстовое реле: он не передаёт `tools`, теряет
`toolCall`/`toolResult` и игнорирует `done.reason`, поэтому модель в Build не может
править файлы, а завершение хода не сверяется с кадрами. Нужно собрать `tools` из
контекста Pi, вернуть результаты с `toolCallId` и сделать `done.reason`
проверяемым. `authorize()` при этом не ослабляется.

## Что делается

1. Импорт `getCurrentTools`/`getCurrentSystemPrompt` из `@earendil-works/pi-ai`
   (сверить фактический экспорт в `package.json`; объявления —
   `dist/utils/transcript.d.ts:26,34`, `Tool` — `dist/types.d.ts:423-428`).
2. `MAX_FRAME_BYTES` в TS — `512 * 1024`.
3. `transcript(context)`: system из `getCurrentSystemPrompt(messages)` если непустой;
   user как есть; assistant — текст + `content.filter(type==="toolCall")` →
   `tool_calls` (пустой текст ⇒ `null`, сообщение с вызовами не выбрасывать);
   `toolResult` → `{tool_call_id: toolCallId, name: toolName, text, is_error: isError}`
   (`dist/types.d.ts:376-386`); не-текстовые блоки —
   `[non-text content omitted: N]`; прочие роли — `user` с видимым `[role]`.
4. Запрос `{kind:"turn", tools: getCurrentTools(messages).map(...), messages}`.
5. `finish` берёт `done.reason`, `tool_use` → `toolUse`, противоречие причина/кадры —
   ошибка.
6. `authorize()` и `pi.on("tool_call")` не менять.
7. `Scripted` в `tests/pi_transport.rs` и `tests/build_runner.rs` получает
   `turn_request(&TurnRequest)` (в pi — с записью имён `tools` и копии `messages`, в
   build_runner — делегирование), иначе умолчание отвергнет непустые tools.
8. В `tests/pi_transport.rs` `case()` (`:130`) список `["bash","read"]` не
   переиспользовать для проверки tools — брать отдельный read/edit/write.

## RED

- `pi_offers_its_tools_on_every_turn` (read/edit/write, без bash; сегодня падает)
- `the_second_turn_carries_the_tool_result_with_its_id` (сегодня падает)
- `a_done_reason_that_contradicts_the_frames_is_an_error` (сегодня падает)
- `a_large_inbound_frame_is_accepted` (>64 КиБ, сегодня падает)
- `pi_executes_an_edit_the_model_asked_for` (регрессия; может быть зелёным сегодня)
- `cargo test -p stellwerk-cli --test build_runner` — все 16 остаются зелёными,
  потому что `Scripted` теперь принимает tools.

## Приёмка

```
cargo test -p stellwerk-task --test pi_transport
cargo test -p stellwerk-cli --test build_runner
```

Ожидание: `pi_transport` ≥ 7 тестов, `build_runner` — все 16. Ручной canary
Plan→Build с реальным `edit`: модель вызывает `edit`, гейт разрешает R1, файл
меняется, `ходов модели` > 1.

## Не делается

`authorize()` и `pi.on("tool_call")` не меняются. Не включается стриминг,
`tool_choice`, `parallel_tool_calls`, `--mode rpc`. Не читается
`~/.pi/agent/auth.json`. Не добавляются зависимости (npm). Результаты инструментов
не обрезаются и не считаются evidence.

## Бюджет

Не больше 3 изменённых файлов. C2: src ≤ 200 логически добавленных строк,
tests ≤ 250 добавленных строк. Новые зависимости не добавляются. `#[allow(...)]`
и `#[ignore]` запрещены.
