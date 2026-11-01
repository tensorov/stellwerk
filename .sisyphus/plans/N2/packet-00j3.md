---
id: N2/00j3
milestone: N2
title: Ход несёт определения инструментов и честную причину
depends_on: [N2/00j1, N2/00j2, 00j2]
allowed_changes:
  - crates/stellwerk-task/src/m5/model_transport.rs
  - crates/stellwerk-task/tests/model_transport_wire.rs
---

# 00j3 — Ход несёт определения инструментов и честную причину

## Зачем

`serve_once` не знает, какие инструменты предложены, поэтому не может отличить
вызов из чужого набора от легального, а `done.reason` — константа `"stop"`, даже
когда ход отправил `Frame::ToolCall`. Нужны три различимых состояния (`None` —
legacy, набор неизвестен; `Some([])` — сознательное «инструментов нет»;
`Some(list)` — список) и причина завершения, выведенная из фактически отправленных
кадров. Иначе отсутствие данных снова превращается в разрешение, и «вызов вне
списка» неотличим от «списка нет».

## Что делается

1. `ToolDef {name, description, parameters: Value}` и
   `TurnRequest {tools: Option<Vec<ToolDef>>, messages}`.
2. `parse` возвращает `TurnRequest`, белый список `kind|tools|messages`.
3. `tools` **необязателен** и различает состояния: `None` — legacy (набор
   неизвестен, валидация вызовов не применяется), `Some([])` — явное
   «инструментов нет» (любой `ToolCall` отвергается), `Some(list)` — вызов вне
   списка отвергается.
4. Каждый `ToolDef`: непустое имя, `parameters` — объект, иначе `Schema`.
5. В трейт добавить `turn_request(&TurnRequest)` с умолчанием: `Some` непустой ⇒
   `BackendError("backend does not accept tools")`, иначе `turn(&messages)`.
6. `serve_once` зовёт `turn_request`; при `Some` проверяет каждый `ToolCall.name` и
   при нарушении пишет **только** `Error{ToolNotOffered}` + `Served::Refused`.
7. `done.reason` = `tool_use` при наличии `ToolCall`, иначе `stop` (это меняет
   ожидание `a_turn_roundtrip_streams_text_tool_and_done` с `stop` на `tool_use`,
   потому что `TURN` содержит bash-вызов).
8. `ErrorCode::ToolNotOffered` (`tool_not_offered`).
9. Добавить `Recording` backend в `tests/model_transport_wire.rs`: реализует `turn`
   и `turn_request`, запоминает **имена** `tools` (не `TurnRequest` целиком:
   `ToolDef`/`TurnMessage` ещё не `Clone`), возвращает сценарий; `TurnOnly` —
   только `turn` для проверки умолчания.
10. Планировщик **не** трогать: его `"tools": []` — отдельный пакет 00j3a.
11. Не менять рендер HTTP и `TurnMessage`.

## RED

- `a_legacy_turn_is_forwarded_without_offer_validation` (`TURN` без tools + bash →
  Text, ToolCall, Done `tool_use`).
- `an_explicit_empty_tool_set_refuses_any_call` (payload `"tools":[]` + bash →
  только `tool_not_offered`).
- `a_call_outside_the_offered_set_is_refused` (`Some([read])` + bash).
- `a_call_inside_the_offered_set_is_forwarded` (`Some([bash])` + bash → Done
  `tool_use`).
- `a_text_only_turn_still_ends_with_stop`.
- `a_backend_without_tool_support_refuses_offered_tools` (`TurnOnly` +
  `Some([read])`).
- `a_turn_roundtrip_streams_text_tool_and_done` меняет ожидание с `stop` на
  `tool_use`, потому что `TURN` содержит bash-вызов.

## Приёмка

```
cargo test -p stellwerk-task --test model_transport_wire
cargo test -p stellwerk-task --test pi_transport
cargo test -p stellwerk-cli --test build_runner
cargo clippy -p stellwerk-task --all-targets --no-deps -- -D warnings
cargo fmt -p stellwerk-task -- --check
```

Ожидание: весь wire-набор зелёный; legacy `Scripted` идёт через умолчание;
`build_runner` не регрессировал; clippy/fmt чисто.

## Не делается

Планировщик не трогается: его `"tools": []` — 00j3a. HTTP-рендер и `TurnMessage`
не меняются. Не добавляются `tool_choice`, `parallel_tool_calls`, стриминг.
Никаких новых зависимостей, `#[allow]`/`#[ignore]`.

## Бюджет

Не больше 3 изменённых файлов. C2: src ≤ 200 логически добавленных строк,
tests ≤ 250 добавленных строк. Новые зависимости не добавляются. `#[allow(...)]`
и `#[ignore]` запрещены.
