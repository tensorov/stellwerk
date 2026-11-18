# N2-00d — протокол кадров и хост-прокси модели (scripted backend)

**label:** — · **mutation_gate:** yes (граница протокола)
**depends_on:** [N2-00c] · **ADR:** не требуется
overlap_waiver: C9 — N1/packet-03 и N2/00d делят crates/stellwerk-task/src/m5/mod.rs, последовательное исполнение

## Цель

Хостовый AF_UNIX-сокет из N2-00c получает **протокол**: одна заявка `turn` на
соединение, ответ — поток кадров `text` / `tool_call` / `done` / `error`. Прокси
живёт на хосте, маршрут выбирает он, а не клиент: полей `url`, `headers`,
`base_url`, `model` в схеме **нет**, и кадр с ними отвергается.

Инвариант:

> Клиент в песочнице не может ни выбрать адресата, ни подсунуть заголовки, ни
> «промолчать» в ответ: неизвестный `kind`, неизвестное поле, битый JSON и
> превышение размера кадра — **разные** отказы с разными `code`, и ни один не
> выглядит как `done`. Отсутствие заявки — состояние `NoRequest`, а не успех.

HTTPS-транспорт и токен здесь **не строятся**: это N2-00g. Backend в этом пакете —
`ScriptedBackend` в тестах; боевой `OpenAiBackend` приходит в 00g, поэтому
`ModelProxy::serve_once` принимает `&dyn ModelBackend` и ничего не знает о сети.

## production_call

- `ModelProxy::bind` / `serve_once` — **библиотечный шов**: живого вызывающего в
  этом пакете нет, проводка в `SandboxWorker::exec` — N2-00f/h (как и в
  N2-00b: доказательство на фикстуре, не на живой сети). Это заявлено явно,
  чтобы «нет вызывающего» не читалось как «уже в продакшене».

## files_allowed

- `crates/stellwerk-task/src/m5/model_transport.rs` (новый)
- `crates/stellwerk-task/src/m5/mod.rs`
- `crates/stellwerk-task/tests/model_transport_wire.rs` (новый)

**В лимите ADR-0003 (3 файла).**

## Что делать

1. Кадры — JSONL, UTF-8, **≤ 64 KiB на кадр**; чтение ограничено: превышение
   отвергается до выделения памяти под весь кадр.
2. Клиент → хост: только `{"kind":"turn","messages":[{"role":"user|assistant","text":"..."}]}`.
   Неизвестный ключ верхнего уровня — `unexpected_field` с именем ключа; схема
   остального (`deny_unknown_fields`) — `schema` с текстом serde-ошибки.
3. Хост → клиент: `text{text}`, `tool_call{id,name,input}`, `done{reason}`,
   `error{code,message}`. `done` и `error` — разные терминальные кадры;
   `error.code` — перечисление `unknown_kind | malformed_json | unexpected_field |
   schema | oversize_frame | backend_error`. Не-UTF-8 и кадр, оборванный до `\n`,
   приходят как `malformed_json` с разным `message`: отказ один и тот же (кадр не
   разобран), различать их отдельными кодами незачем.
4. `ModelProxy::bind(dir)` создаёт `model.sock` в **хозяйском** каталоге, отдаёт
   `socket()` и удаляет файл в `Drop`. `serve_once(&dyn ModelBackend)` принимает
   одно соединение, обслуживает **одну** заявку и закрывает: вторая заявка на том
   же соединении не исполняется.
5. `Served::{Turn, NoRequest, Refused}` — три состояния, не `bool`: «обслужил»,
   «клиент подключился и ушёл», «кадр отвергнут с error-кадром».
6. Тесты (`tests/model_transport_wire.rs`):
   - `a_turn_roundtrip_streams_text_tool_and_done` — 3 кадра, терминальный `done`;
   - `an_unknown_kind_is_refused_not_ignored` — `unknown_kind`, без `text`/`done`;
   - `a_route_field_is_refused` — `url` (и `headers`/`base_url`/`model`) дают
     `unexpected_field` с именем поля;
   - `malformed_json_is_refused` — `{` → `malformed_json`;
   - `an_oversize_frame_is_refused_before_it_is_buffered` — > 64 KiB без `\n` →
     `oversize_frame`;
   - `a_backend_error_is_an_error_frame_not_done` — `backend_error`, без `done`;
   - `a_connection_without_a_request_is_no_request` — `Served::NoRequest`, кадров нет;
   - `a_second_turn_on_one_connection_is_not_served` — ровно один ответ на соединение;
   - `a_frame_at_the_size_limit_is_served_not_refused` — кадр ровно 64 KiB вместе с
     `\n` обслуживается: предел строгий, «больше», а не «не меньше»;
   - `a_dropped_proxy_removes_its_socket_file` — после `Drop` файла сокета нет.

## Почему `mutation_gate: yes`

Мутант, который игнорирует неизвестный `kind`, принимает `url` в схеме, шлёт
`done` вместо `error` или читает кадр без предела, превращает протокол в
открытый ретранслятор. Тест обязан удерживать каждое из этих свойств.

## Запрещено трогать

`secrets.rs` (`TokenProxy` — N2-00g), `sandbox_bwrap.rs` (готов в 00c),
`runner.rs`, `gateway::decide` / `RISK_PERMISSION`, `evidence.rs`, store,
`justfile`, `.github/`, `scripts/plan-lint.py`, корневой `Cargo.toml`,
`modules.lock`. Новых зависимостей нет: `serde`/`serde_json` уже подключены;
сеть не используется вовсе — ни `std::net`, ни HTTP-клиента.

## Приёмка

```sh
cargo build --workspace --all-targets; echo "BUILD_EXIT=$?"
cargo test -p stellwerk-task --test model_transport_wire; echo "TEST_EXIT=$?"
cargo fmt -p stellwerk-task -- --check; echo "FMT_EXIT=$?"
cargo mutants -p stellwerk-task --file crates/stellwerk-task/src/m5/model_transport.rs \
  -- --test model_transport_wire; echo "MUTANTS_EXIT=$?"
grep -n "std::net\|reqwest\|hyper\|ureq" crates/stellwerk-task/src/m5/model_transport.rs  # пусто
```

**Все четыре `0`** (у `mutants` допустим код 2 при ≥75% пойманных и явно названном
выжившем). Отказ проверяется `code` в error-кадре, а не отсутствием `text`:
отсутствие кадра неотличимо от «кадр записать забыли».
