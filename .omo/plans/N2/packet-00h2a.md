# N2-00h2a — канал планировщика: ход модели без исполнения

**label:** — · **mutation_gate:** no · **depends_on:** [N2-00h1, 00h1]

## Цель

Дать планировщику настоящий ход модели через наш транспорт. Планирование ничего не
исполняет: модель возвращает текст, инструментов нет, поэтому ни песочницы, ни гейта на этом
шаге не нужно — они появляются в 00h3, где есть исполнение.

Важно: это не «урезанный» канал. Ход идёт тем же путём, что и в Build —
`planner → model.sock → ModelProxy → HttpBackend → сервис`, с теми же кадрами и с тем же
токеном, который не покидает прокси.

## files_allowed

- `crates/stellwerk-cli/src/planner_host.rs` (новый)
- `crates/stellwerk-cli/src/lib.rs` (`pub mod planner_host;`, ≤2 строки)
- `crates/stellwerk-cli/tests/planner_host.rs` (новый)

## Что делать

1. `pub struct PlannerHost { model: HostModel }`.
2. `PlannerHost::prepare(config, scratch, visible) -> Result<Self, PlannerError>` — ровно
   `HostModel::prepare`, без второй логики настройки.
3. `pub fn ask(&self, prompt: &str) -> Result<String, PlannerError>`:
   - поток на `model.serve_once()` (прокси обслуживает одно соединение за вызов);
   - `UnixStream::connect(model.socket())`, `set_read_timeout(120s)`;
   - заявка `{"kind":"turn","messages":[{"role":"user","text": prompt}]}` + `\n`;
   - читать построчно: `text` копить, `error` → `PlannerError::Refused { code, message }`,
     `done` → стоп; `tool_call` в плане — `PlannerError::UnexpectedTool` (планировщик
     инструментов не запрашивал и запросить не может, это дефект маршрута, а не план);
   - нет `text` и нет отказа → `Channel`: это оборванный протокол, а не пустой план;
   - после `done` — join потока прокси, чтобы `Done` был записан до возврата.
4. `PlannerError` — `Model(HostModelError)`, `Channel(String)`,
   `Refused { code: String, message: String }`, `UnexpectedTool`. `Display` без токена
   (текст приходит из уже смытых отказов). Отдельного `NoText` нет: пустой `content`
   отвергает backend (пустой ход неотличим от «сервис не ответил»), а оборванный ответ без
   отказа — дефект протокола, то есть `Channel`, а не свойство модели.
5. Два хода подряд на одном сокете обязаны работать: TUI повторяет вопрос по `r`.

## Тесты (`tests/planner_host.rs`), 6

- `one_turn_returns_the_model_text` — локальный HTTP-сервер, `ask("цель")` → «план готов»,
  в запросе проверен `POST /v1/chat/completions`;
- `two_turns_share_one_socket` — второй `ask` после первого, оба ответа дошли;
- `a_failed_status_is_a_refusal_not_an_empty_plan` — `500` → `Refused` с кодом;
- `an_empty_message_is_refused_rather_than_becoming_an_empty_plan` — пустой `content` →
  `Refused` (пустоту отвергает backend, а не планировщик);
- `the_token_never_appears_in_the_refusal` — сервер отвечает `401` с эхом значения токена,
  в тексте отказа значения нет;
- `a_missing_socket_is_a_channel_error` — сокет убран до хода → `Channel`, без паники.

Сервер и сокет — как в 00h1 (потолок приёма 5 с, `read_timeout` на клиенте).

## Приёмка

```
cargo test -p stellwerk-cli --test planner_host
cargo test -p stellwerk-cli
cargo clippy -p stellwerk-cli --all-targets --no-deps -- -D warnings
cargo fmt -p stellwerk-cli -- --check
```

C2: `src` ≤ 200 логических строк (факт: 112), `tests` ≤ 250 (факт: 175), 3 файла.

## Запрещено

Менять `host_model.rs`, `plan.rs`, `BUILD_UNAVAILABLE_REASON` (это 00h2b); запускать Pi;
добавлять зависимости; ретраить отказ транспорта молча.
