# N2-00g2 — `TokenProxy::send` получает тело (граница секрета)

**label:** `contract-change` (меняется граница секрета) · **mutation_gate:** no (обоснование ниже)
**depends_on:** [N2-00g1] · **ADR:** `docs/adr/0020-http-transport.md` (написан в 00g1, здесь не меняется)

## Цель

Сейчас `TokenProxy::send` умеет только `METHOD path HTTP/1.1` + `authorization`. Для
`POST /v1/chat/completions` нужны `host`, `content-type`, `content-length` и тело. Тело
вставляет вызывающий, **токен по-прежнему вставляет только прокси**: у `Token` нет ни
`Display`, ни аксессора, и этот пакет их не добавляет.

## Что делать (`src/m5/secrets.rs`)

1. Новый тип запроса:

```rust
pub struct ProxyRequest<'a> {
    pub method: &'a str,          // "POST"
    pub host: &'a str,            // "api.example.ru" или "127.0.0.1:8080"
    pub path: &'a str,            // "/v1/chat/completions", обязан начинаться с '/'
    pub content_type: Option<&'a str>,
    pub body: Option<&'a str>,
}
```

2. `send` принимает `ProxyRequest<'_>` вместо `(&str, &str)`; сигнатура transport не меняется
   (`FnOnce(&str) -> Result<(u16, String), String)`).

3. Wire — фиксированный порядок заголовков, ровно так:

```
{method} {path} HTTP/1.1\r\n
host: {host}\r\n
[content-type: {ct}\r\n]
[content-length: {len}\r\n]
authorization: Bearer {value}\r\n
\r\n
[body]
```

4. Новые отказы, каждый — отдельный вариант, а не общий `Invalid`:

```rust
EmptyRequestField { field: &'static str },   // method / host / path пусты
PathNotAbsolute,                              // path не начинается с '/'
BodyWithoutContentType,                       // тело без типа: сервис прочитает как мусор
BodyTooLarge { len: usize, max: usize },      // потолок 1 MiB
```

5. `MAX_BODY: usize = 1024 * 1024` — публичная константа, чтобы тест и backend ссылались на
   одно число, а не на литерал.

6. Смывка ответа не меняется: `ProxyResponse.body` и `ProxyError::Upstream` проходят
   `Token::redact` как раньше. **Тело запроса не логируется и не смывается** — это данные
   вызывающего, а не секрет; единственное значение, которое прокси подставляет, — токен.

7. `Token`, `AgentEnv`, `agent_argv`, `read_token_outside` не меняются.

## Тесты (`tests/secrets.rs`)

Существующие тесты переводятся на новую сигнатуру, их число не уменьшается; добавляются три:

- `wire_carries_host_content_type_and_body` — точное сравнение всей строки, включая порядок
  заголовков и `content-length` в байтах UTF-8 (не в символах);
- `body_without_content_type_refused` — `BodyWithoutContentType`, transport не вызван;
- `oversize_body_refused_before_transport` — `MAX_BODY + 1` байт → `BodyTooLarge`, transport
  не вызван (иначе отказ был бы уже после отправки).

Плюс один на границу: `authorization_header_is_added_exactly_once` — заголовок с токеном
встречается в wire ровно один раз, вставка вызывающим невозможна (в `ProxyRequest` нет поля).

## Почему `mutation_gate: no`

Изменение — сборка строки с фиксированным порядком; его ловит точное сравнение wire
(`wire_carries_host_content_type_and_body`), а каждая новая ветка отказа — отдельный тест с
проверкой «transport не вызван». Логики, где мутант выжил бы незамеченным, пакет не
добавляет.

## files_allowed

- `crates/stellwerk-task/src/m5/secrets.rs`
- `crates/stellwerk-task/tests/secrets.rs`

**В лимите ADR-0003 (2 файла).**

## Приёмка

```
cargo test -p stellwerk-task --test secrets
cargo test -p stellwerk-task
cargo clippy -p stellwerk-task --all-targets --no-deps -- -D warnings
cargo fmt -p stellwerk-task -- --check
```

C2: `src` ≤ 200 логических строк, `tests` ≤ 250 строк.

## Запрещено

Добавлять аксессор к `Token` или `Display`; печатать тело запроса; менять `send` так, чтобы
заголовок авторизации можно было задать снаружи; трогать `model_transport.rs`,
`gate_channel.rs`, `sandbox_bwrap.rs`.
