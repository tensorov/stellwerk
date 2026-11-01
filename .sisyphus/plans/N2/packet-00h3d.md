# N2-00h3d — провод `b`: прогон Build из TUI, без токена не начинается

**label:** — · **mutation_gate:** no · **depends_on:** [N2-00h3c, 00h3a, 00h3c]

## Что здесь доказывается и чего нет

Доказывается: клавиша `b` в `Stage::Approved` **запускает прогон** — допуск проверяется тем же
`build_request`, бэкенд собирается по реальному пути токена, прогон уходит в отдельный поток,
итог (счётчики и вывод агента) возвращается в TUI, а не теряется.

Доказывается отдельно, что **отсутствие токена — это отказ начать**, с названным файлом, а не
бэкенд, который «потом как-нибудь» и не «сборка не удалась».

Не доказывается: живой ход модели. Токена в окружении нет — N2-00a.

## files_allowed

- `crates/stellwerk-cli/src/plan.rs`
- `crates/stellwerk-cli/src/build_runner.rs`
- `crates/stellwerk-cli/tests/plan.rs`

## Что делать

1. `sandbox_roots()` в `build_runner` — один список на песочницу и на проверку пути токена:
   `/usr /bin /lib /lib64 /etc`. Токен под ними лежать не может, иначе прокси обходится
   чтением файла.
2. `build_backend_at(path)` / `build_backend()` в `plan.rs`: токен читается
   `read_token_outside` под этими корнями, `HttpBackend::new(HOST_BASE_URL, HOST_MODEL_ID, …)`.
   `build_backend()` берёт путь по правилу владельца (`ModelConfig::from_env`).
3. `Stage::Building` + строка «агент работает в копии дерева… (прогон не отменяется)».
   `PlanUi.build: Option<Receiver<Result<BuildOutcome, String>>>`.
4. `start_build`: `STELLWERK_PI_EXTENSION` (путь к `stellwerk_transport.ts`) и `PI_BIN`
   (иначе `$HOME/.pi/agent/bin/pi`) — обязательны, без угадывания; бэкенд собирается
   **до** потока, поэтому «нет токена» печатается как «Build не запущен», а не как
   неудачный прогон; корень прогона — `temp_dir/stellwerk-build-<pid>`.
5. `drain_build` в цикле: `Ok(outcome)` → счётчики + первые 240 символов вывода,
   `Err` → `Stage::Failed`, `Disconnected` → отдельная причина («поток сборки исчез
   без ответа»), потому что это не пустой вывод.
6. `BUILD_UNAVAILABLE_REASON` переписан: исполнение подключено, блокирует токен и его путь.

## Тест (`tests/plan.rs`), 1 новый

- `the_build_names_the_missing_token` — `build_backend_at(absent)` отказывает, в причине
  и имя файла, и полный путь. `Ok` в этом тесте — провал.

## Приёмка

```
cargo test -p stellwerk-cli --test plan
cargo test -p stellwerk-cli
cargo clippy -p stellwerk-cli --all-targets --no-deps -- -D warnings
cargo fmt -p stellwerk-cli -- --check
```

C2: `src` ≤ 200 логических, `tests` ≤ 250, 3 файла.

## Запрещено

Ходить в сеть, читать `auth.json`, угадывать путь расширения сборки (`CARGO_MANIFEST_DIR`
в бинаре), печатать токен, выдавать «Build допущен» за «Build запущен», смешивать
«нет токена» с «сборка упала», трогать owner-WIP.
