# N2-00g3a — провод: адрес, TLS и чтение ответа

**label:** — · **mutation_gate:** no (провод покрыт интеграционными тестами на реальном сокете)
**depends_on:** [N2-00g2, 00e] · **ADR:** не требуется
overlap_waiver: C9 — N1/packet-03 и N2/00g3 делят crates/stellwerk-task/src/m5/mod.rs, последовательное исполнение

## Цель

Отделить байты от политики: `src/m5/http_wire.rs` знает, как разобрать базовый адрес,
соединиться (TLS или loopback-HTTP) и прочитать ответ по `content-length`. Он не знает ни
`Frame`, ни модели, ни токена — на вход получает готовый wire от `TokenProxy`.

Разделение вынужденное: backend и провод вместе дали 263 логических строки при потолке 200,
то есть `SPLIT_REQUIRED` по C2.

## files_allowed

- `crates/stellwerk-task/src/m5/http_wire.rs` (новый)
- `crates/stellwerk-task/src/m5/mod.rs` (+2 строки)
- `crates/stellwerk-task/tests/http_wire.rs` (новый)

## Что делать

1. `BaseUrl::parse(base_url) -> Result<Self, String>`: схема (`https`/`http`), хост, порт
   (443/80 по умолчанию), префикс пути. Иные схемы — отказ.
2. **`http` разрешён только для loopback** (`localhost`, `127.0.0.1`, `::1`): иначе
   `plain http is allowed only on loopback…`. Тестовый путь не должен становиться способом
   отправить токен открытым текстом.
3. `BaseUrl::authority()` — `host:port` для заголовка `host`; `BaseUrl::path(tail)` —
   префикс + хвост.
4. `BaseUrl::exchange(wire) -> Result<(u16, String), String>`: соединение, запись целиком,
   чтение до конца тела по `content-length`. `read_to_end` не используется: он зависел бы на
   закрытие соединения, а сервис вправе держать его открытым.
5. Отказы отдельными текстами: `chunked` не поддерживается; нет `content-length`; соединение
   закрыто до конца тела; заголовки больше 64 KiB; нет строки статуса.
6. TLS: `rustls` + `webpki_roots::TLS_SERVER_ROOTS`, SNI из host, проверка сертификата
   обязательна, `dangerous()` не используется.

## Тесты (`tests/http_wire.rs`), 5

- `the_base_url_yields_authority_and_path_with_the_prefix` — порт по умолчанию и префикс;
- `plain_http_outside_loopback_is_refused` — `example.ru` и `ftp://` отвергнуты;
- `an_exchange_returns_the_status_and_the_declared_body` — реальный сокет, код и тело;
- `a_chunked_response_is_refused`;
- `a_response_without_content_length_is_refused`.

Сервер в тестах принимает соединение **с потолком 5 с**: backend, который никуда не пошёл,
обязан провалить тест, а не держать `accept` до общего таймаута.

## Приёмка

```
cargo test -p stellwerk-task --test http_wire
cargo test -p stellwerk-task
cargo clippy -p stellwerk-task --all-targets --no-deps -- -D warnings
cargo fmt -p stellwerk-task -- --check
```

C2: `src` ≤ 200 логических строк (факт: 154 + 2), `tests` ≤ 250 (факт: 97), 3 файла.

## Запрещено

Трогать `secrets.rs`, `model_transport.rs`, `sandbox_bwrap.rs`; отключать проверку
сертификата; разрешать `http` вне loopback; добавлять зависимости вне 00g1.
