# N2-00h1 — `HostModel`: адрес, токен и сокет в одном месте

**label:** — · **mutation_gate:** no (сборка конфигурации; поведение проверяется тестами отказов)
**depends_on:** [N2-00g3b] · **ADR:** не требуется (ADR-0020 уже описывает транспорт)

## Цель

Собрать production-точку входа к модели: из трёх внешних величин (базовый адрес, id модели,
файл токена) получить готовую пару `ModelProxy` + `HttpBackend`. Пакет **не** трогает TUI:
`LiveTransport` в `plan.rs` остаётся на `opencode`, а `BUILD_UNAVAILABLE_REASON` остаётся
правдой, пока Build не подключён (00h2/00h3).

## Что требуется на входе (и почему это не дефолт)

| величина | источник | поведение при отсутствии |
|---|---|---|
| `base_url` | конфигурация вызывающего; владелец подтвердил `https://api.neuraldeep.ru/v1` | **отказ** `Unconfigured { field: "base_url" }` |
| `model_id` | `ModelBinding.model_id` из контракта — голое `qwen3.6-unlim-xl-noreason` | **отказ** `Unconfigured { field: "model_id" }` |
| файл токена | `$XDG_CONFIG_HOME/stellwerk/neuraldeep.token` (нет `XDG_CONFIG_HOME` → `$HOME/.config/...`) | **отказ** `Unreadable` |

Адрес подтверждён владельцем и потому известен, но в код дефолтом не зашивается: значение
приходит параметром, пустое — отказ. В теле запроса уходит **голый** `model_id`
(`qwen3.6-unlim-xl-noreason`), без префикса провайдера: префикс — это маршрут Pi
(`neuraldeep/...`), а не поле OpenAI-совместимого API; так же записан `ModelBinding` в
`tests/pi_transport.rs`. Проверяется первым живым ходом (canary), а не молчаливым выбором.

Дефолтного адреса у пакета нет намеренно: придуманный `api.<provider>.ru` выглядел бы как
рабочая настройка и упал бы на живом прогоне уже после того, как человек утвердил план.
Пустая строка — тоже отказ, а не «возьмём что-нибудь».

## files_allowed

- `crates/stellwerk-cli/src/host_model.rs` (новый)
- `crates/stellwerk-cli/src/lib.rs` (`pub mod host_model;` + реэкспорт, ≤4 строки)
- `crates/stellwerk-cli/tests/host_model.rs` (новый)

## Что делать

1. `pub struct ModelConfig { base_url: String, model_id: String, token_path: PathBuf }` —
   конструктор `ModelConfig::from_env(model_id, base_url)` вычисляет путь токена из
   `XDG_CONFIG_HOME`/`HOME`; пустой `base_url`/`model_id` → `Unconfigured`.
2. `pub struct HostModel { proxy: ModelProxy, backend: HttpBackend }` либо `HostModel::prepare(config, scratch, visible)`,
   который:
   - читает токен `read_token_outside(&config.token_path, visible, "NEURALDEEP_TOKEN")`;
   - строит `TokenProxy::new(token)`;
   - `HttpBackend::new(&config.base_url, &config.model_id, proxy)`;
   - `ModelProxy::bind(scratch)`.
3. Геттеры: `socket() -> &Path`, `serve_once(&self) -> io::Result<Served>` (делегирует в
   `ModelProxy`, который сам добавляет `Frame::Done`).
4. `#[derive(Debug)]` на `HostModel` допустим только если `HttpBackend`/`TokenProxy` не печатают
   значение токена — проверить тестом, а не памятью.
5. `Drop` прокси уже удаляет сокет — не дублировать.

## Тесты (`tests/host_model.rs`), 5

- `a_missing_base_url_is_an_explicit_unconfigured_state_not_a_default` — пустая строка и
  отсутствие поля различимы по имени поля;
- `a_token_file_inside_a_visible_root_is_refused` — `ConfigInsideSandbox`, значение не
  появляется в тексте;
- `the_debug_of_a_prepared_model_carries_no_token`;
- `the_socket_lives_in_the_scratch_directory_and_disappears_on_drop`;
- `one_turn_goes_through_the_proxy_and_the_proxy_adds_done` — заглушка `ModelBackend` вместо
  сети: один `Text`, затем `Done { reason: "stop" }`.

## Приёмка

```
cargo test -p stellwerk-cli --test host_model
cargo test -p stellwerk-cli
cargo clippy -p stellwerk-cli --all-targets --no-deps -- -D warnings
cargo fmt -p stellwerk-cli -- --check
```

C2: `src` ≤ 200 логических строк, `tests` ≤ 250, 3 файла.

## Запрещено

Читать `~/.pi/agent/auth.json`; печатать токен или тело запроса; менять `plan.rs` и
`BUILD_UNAVAILABLE_REASON` (это 00h2); добавлять зависимости; подставлять дефолтный base URL.
