# N2-00g3b — `HttpBackend`: кадр `turn` → chat/completions → кадры

**label:** — · **mutation_gate:** yes (≥75% caught, `--file src/m5/model_backend.rs`)
**depends_on:** [N2-00g3a] · **ADR:** не требуется

## Цель

Реализовать `ModelBackend` (`model_transport.rs`, трейт не меняется) поверх провода N2-00g3a:

```rust
fn turn(&self, messages: &[TurnMessage]) -> Result<Vec<Frame>, BackendError>;
```

так, чтобы живой ход модели шёл с хоста: `Pi → model.sock → ModelProxy → HttpBackend →
api.neuraldeep.ru`. В песочнице по-прежнему `--unshare-net`, токен в неё не попадает.

**`Frame::Done` backend не добавляет** — трейт говорит прямо: терминальный `done` добавляет
прокси, backend не решает, закончился ли ход.

## files_allowed

- `crates/stellwerk-task/src/m5/model_backend.rs` (новый)
- `crates/stellwerk-task/tests/model_backend.rs` (новый)

## Что делать

1. `HttpBackend::new(base_url, model_id, token)`: адрес разбирает `BaseUrl` (00g3a), неверный
   адрес — отказ до первого хода.
2. `turn` собирает тело:

```json
{"model":"<model_id>","messages":[{"role":"user","content":"…"}],"stream":false}
```

   роль — из `TurnMessage` (`User`/`Assistant`), порядок сообщений сохраняется.
3. Отправка — `token.send(ProxyRequest { …, body }, |wire| base.exchange(wire))`.
4. Ответ `2xx` → разбор `choices[0].message`: непустой `content` → `Frame::Text`;
   `tool_calls[*]` → `Frame::ToolCall { id, name, input }`; `arguments` разбираются в `Value`,
   неразобранные — отказ, а не `{}`.
5. `choices` пуст или отсутствует — `BackendError`: это «сервис не ответил», а не «ход
   закончился» (закон 6). Пустой набор кадров — тоже отказ.
6. Не-`2xx` → `BackendError` с кодом и уже смытым телом.

## Тесты (`tests/model_backend.rs`), 5

Локальный `TcpListener` на loopback (провод уже проверен в 00g3a отдельно):

- `a_text_turn_reaches_the_service_as_chat_completions` — путь, `host`, тип, `authorization`,
  тело и роль; ответ → `Frame::Text`;
- `tool_calls_become_frames_and_their_arguments_are_parsed` — два вызова с `id`/`name`/`input`;
- `a_failed_status_carries_the_code_and_never_the_token` — `401` с эхом токена: код есть,
  значения нет;
- `a_malformed_response_is_an_error_not_an_empty_turn`;
- `empty_choices_are_an_error_not_a_finished_turn`.

Сервер принимает соединение с потолком 5 с: мутант, при котором `turn` не идёт в сеть,
обязан провалить тест, а не висеть (иначе `cargo-mutants` считает его timeout, а не caught).

## Мутанты

`cargo mutants -p stellwerk-task --file crates/stellwerk-task/src/m5/model_backend.rs -- --test model_backend`

Факт: 7 мутантов — **4 caught, 3 unviable, 0 missed** (100% от caught+missed).

## Известные пробелы (переносятся в 00h/P5)

- `usage` ответа не учитывается: `budget_tokens_per_task` из `ModelBinding` пока не
  расходуется — бюджета ещё нет ни у кого, и выдумывать счётчик значило бы объявить его
  проверенным.
- `--mode json` для живого запуска Pi: `pi_transport_args` режим не задаёт (пробел 00f),
  закрывается в 00h вместе с проводкой CLI.
- `HttpBackend` пока не вызывается из production-кода: `ModelProxy` в CLI получает его в 00h.
  До этого `BUILD_UNAVAILABLE_REASON` остаётся правдой.
- Путь токена зафиксирован владельцем: `$XDG_CONFIG_HOME/stellwerk/neuraldeep.token` (права
  `0600`), читается `read_token_outside` в 00h; `~/.pi/agent/auth.json` не читается и не
  монтируется.

## Приёмка

```
cargo test -p stellwerk-task --test model_backend
cargo test -p stellwerk-task
cargo build --workspace --all-targets
cargo clippy -p stellwerk-task --all-targets --no-deps -- -D warnings
cargo fmt -p stellwerk-task -- --check
```

C2: `src` ≤ 200 логических строк (факт: 122), `tests` ≤ 250 (факт: 195), 2 файла.

## Запрещено

Менять `model_transport.rs`, `secrets.rs`, `http_wire.rs`, `gate_channel.rs`,
`sandbox_bwrap.rs`, `contract.rs`; добавлять зависимости вне 00g1; логировать тело запроса.
