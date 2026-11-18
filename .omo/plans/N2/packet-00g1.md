# N2-00g1 — TLS-транспорт для хостового backend (зависимости)

**label:** `contract-change` · **mutation_gate:** no (только манифесты и ADR; кода нет)
**depends_on:** [N2-00f] · **ADR:** `docs/adr/0020-http-transport.md` (входит в пакет)
overlap_waiver: C9 — N1/packet-08 и N2/00g1 делят crates/stellwerk-task/Cargo.toml, последовательное исполнение

## Цель

Хостовому backend нужен HTTPS-клиент. Значение токена приватно внутри `Token`
(`secrets.rs` не отдаёт его наружу ни через `Display`, ни через аксессор), поэтому
единственный законный способ отправить запрос — передать transport-замыканию **готовый
HTTP-wire**, в который `TokenProxy::send` сам вставляет `authorization: Bearer …`.
Ни `ureq`, ни `reqwest` такого шва не дают: они строят запрос сами и до токена не
дотянутся, иначе как через новую дыру в `secrets.rs`. Значит транспорт остаётся нашим,
а из внешнего берётся только TLS.

## Изменение зависимостей

Корневой `Cargo.toml`, `[workspace.dependencies]`, с пометкой `⚠️ contract-change (N2-00g1)`:

```toml
rustls       = { version = "0.23", default-features = false, features = ["ring", "std", "tls12"] }
webpki-roots = "1.0"
```

`crates/stellwerk-task/Cargo.toml`, `[dependencies]`:

```toml
rustls        = { workspace = true }
webpki-roots  = { workspace = true }
```

Правила:

1. **`default-features = false`.** Дефолт `rustls` 0.23 тянет `aws-lc-rs` (cmake/nasm) —
   сборочная зависимость хоста, которой в проекте нет. `ring` собирается `cc`, он уже
   требуется транзитивно.
2. **`tls12` включён явно.** Провайдер может не поддерживать TLS 1.3; отказ на 1.2 читался
   бы как «сервис недоступен», а это другое состояние.
3. **`native-tls`/OpenSSL не берём.** Системный CA-store и версия OpenSSL становятся
   частью поведения; проверка сертификата должна быть одинаковой на dev-станции и в CI.
4. **`webpki-roots`, а не `rustls-native-certs`.** Корни — данные в крейте, а не чтение
   `/etc/ssl` во время выполнения: непрочитанный store иначе стал бы «нет доверия» без
   причины (закон 6).
5. **Точная версия фиксируется в `Cargo.lock`** (`cargo update -p rustls --precise 0.23.x`);
   дублей `rustls` в дереве быть не должно.
6. Никаких features `logging`, `read_buf` и прочих: дерево зависимостей — часть диффа.

## Почему не системный `curl`

`curl` не принимает готовый HTTP-запрос как байты: он собирает запрос из флагов, то есть
токен пришлось бы отдавать ему отдельным аргументом или файлом — ровно тот канал, который
`secrets.rs` закрывает. Плюс версия `curl` на машине становится частью контракта.

## files_allowed

- `Cargo.toml` (корневой, только `[workspace.dependencies]`)
- `crates/stellwerk-task/Cargo.toml`
- `docs/adr/0020-http-transport.md` (новый)

**В лимите ADR-0003 (3 файла).** `src`-строк 0.

## Приёмка

```
cargo build -p stellwerk-task
cargo tree -p stellwerk-task -i rustls
cargo tree -p stellwerk-task -d          # нет двух версий rustls
cargo test -p stellwerk-task
```

`cargo tree -p stellwerk-task -i rustls` должен показать ровно одну версию; `tokio`
получает **те же** features, что и раньше (никаких `net`/`tls` — транспорт синхронный,
`std::net`).

## Запрещено

Менять прочие `[workspace.dependencies]`, добавлять фичи `tokio`, трогать `modules.lock`,
`contract.rs`, `.github/`. `cargo add` не используется: строки пишутся руками, чтобы дифф
был виден в ревью.
