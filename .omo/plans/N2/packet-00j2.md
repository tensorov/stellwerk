---
id: N2/00j2
milestone: N2
title: Кадр вмещает транскрипт с инструментами
depends_on: [00d, 00i1]
allowed_changes:
  - crates/stellwerk-task/src/m5/model_transport.rs
  - crates/stellwerk-task/tests/model_transport_wire.rs
  - crates/stellwerk-task/tests/gate_channel.rs
---

# 00j2 — Кадр вмещает транскрипт с инструментами

## Зачем

Потолок кадра 64 КиБ не вмещает транскрипт с результатом `read`: один кадр `turn` с
инструментами и результатом превышает лимит, и честный обмен обрывается на
`OversizeFrame`. Потолок должен быть общим для model proxy и gate channel, иначе
транспорт и гейт разойдутся. Обрезать результаты нельзя: превышение остаётся
честным `oversize_frame`, а не молчаливой потерей данных.

## Что делается

1. `MAX_FRAME_BYTES = 512 * 1024` (`model_transport.rs:21`), комментарий
   «512 КиБ, общий для model proxy и gate channel».
2. `a_frame_at_the_size_limit_is_served_not_refused` собирает кадр
   `payload.resize(512 * 1024 - 1, b' '); payload.push(b'\n')` — литерал 512 КиБ,
   не только константа.
3. `an_oversize_frame_is_refused_before_it_is_buffered` и `tests/gate_channel.rs`
   используют `MAX_FRAME_BYTES + 1`.
4. Переименовать `an_oversize_gate_frame_is_refused` в
   `a_gate_frame_over_the_limit_is_refused`.
5. `gate_channel.rs` **не** меняется: он импортирует константу и получает новый
   потолок автоматически.
6. Не обрезать сообщения, не включать стриминг.

## RED

- `a_frame_at_the_size_limit_is_served_not_refused` — at-limit (`512 * 1024` байт
  включая `\n`) даёт `Served::Turn`.
- `an_oversize_frame_is_refused_before_it_is_buffered` — `MAX_FRAME_BYTES + 1`
  даёт `OversizeFrame`.
- `a_gate_frame_over_the_limit_is_refused` — переименованный
  `an_oversize_gate_frame_is_refused`, `MAX_FRAME_BYTES + 1`.

## Приёмка

```
cargo test -p stellwerk-task --test model_transport_wire --test gate_channel
cargo clippy -p stellwerk-task --all-targets --no-deps -- -D warnings
cargo fmt -p stellwerk-task -- --check
```

Ожидание: at-limit (`512 * 1024` байт включая `\n`) — `Served::Turn`; oversize
(`MAX_FRAME_BYTES + 1`) — `OversizeFrame`; `a_gate_frame_over_the_limit_is_refused`
(`MAX + 1`); clippy/fmt чисто.

## Не делается

Сообщения не обрезаются и не суммаризируются; стриминг не включается.
`src/m5/gate_channel.rs` не меняется. Никаких новых зависимостей,
`#[allow]`/`#[ignore]`.

## Бюджет

Не больше 3 изменённых файлов. C2: src ≤ 200 логически добавленных строк,
tests ≤ 250 добавленных строк. Новые зависимости не добавляются. `#[allow(...)]`
и `#[ignore]` запрещены.
