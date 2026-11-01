---
id: N2/00j3a
milestone: N2
title: Планировщик явно не предлагает инструменты
depends_on: [N2/00j3, 00h2a]
allowed_changes:
  - crates/stellwerk-cli/src/planner_host.rs
  - crates/stellwerk-cli/tests/planner_host.rs
---

# 00j3a — Планировщик явно не предлагает инструменты

## Зачем

После 00j3 ход может нести `tools`, и молчание планировщика стало бы неотличимо от
legacy-клиента с неизвестным набором. Планировщик обязан явно сказать
«инструментов нет» (`"tools": []`), оставаясь без инструментов; `tool_call` от него
по-прежнему ошибка. Явное `[]` — сознательное решение, а не отсутствие данных.

## Что делается

1. Вынести сборку кадра в `fn turn_frame(prompt: &str) -> Value` —
   `{"kind":"turn","tools":[],"messages":[{"role":"user","text":prompt}]}`.
2. `ask` использует `turn_frame`.
3. `read_reply` по-прежнему отвергает `tool_call` (`:118`).
4. Unit-тесты в `planner_host.rs`: `the_planner_sends_an_explicit_empty_tool_set`
   проверяет `frame["tools"] == json!([])`; `a_tool_call_is_still_unexpected` через
   `UnixStream::pair` проверяет `UnexpectedTool`.
5. В `tests/planner_host.rs` добавить **только**
   `the_planner_does_not_forward_tools_to_the_service`: HTTP-тело не содержит
   `"tools"` — ключ опускается и в 00j3a, и в 00j4, потому что `HttpBackend` не
   рендерит `Some([])`.
6. **Не** проверять `"tools":[]` в HTTP: это невозможно по контракту 00j4.
7. Не давать планировщику инструменты, не менять его промпт.

## RED

- unit `the_planner_sends_an_explicit_empty_tool_set`
- unit `a_tool_call_is_still_unexpected`
- integration `the_planner_does_not_forward_tools_to_the_service`

## Приёмка

```
cargo test -p stellwerk-cli --lib planner_host
cargo test -p stellwerk-cli --test planner_host
cargo clippy -p stellwerk-cli --all-targets --no-deps -- -D warnings
cargo fmt -p stellwerk-cli -- --check
```

Ожидание: `2 tests` в `--lib planner_host`, `7 tests` в `--test planner_host`;
clippy/fmt чисто.

## Не делается

Планировщику не даются инструменты, его промпт не меняется. В HTTP не проверяется
`"tools":[]` — по контракту 00j4 ключ опускается. Никаких новых зависимостей,
`#[allow]`/`#[ignore]`.

## Бюджет

Не больше 3 изменённых файлов. C2: src ≤ 200 логически добавленных строк,
tests ≤ 250 добавленных строк. Новые зависимости не добавляются. `#[allow(...)]`
и `#[ignore]` запрещены.
