# N2-00h2b — выбор режима: `host` или `opencode`, и запись выбора в сессию

**label:** — · **mutation_gate:** no · **depends_on:** [N2-00h2a]
overlap_waiver: C9 — N1 и TUI делят crates/stellwerk-cli/src/main.rs, последовательное исполнение

## Цель

Подключить канал планировщика к TUI и сделать выбор режима явным и записанным:

- `stellwerk plan <repo> --planner host` (по умолчанию) — ход через наш транспорт
  (`PlannerHost`, 00h2a);
- `stellwerk plan <repo> --planner opencode` — нынешний `opencode run`, остаётся как
  путь отладки промпта;
- режим пишется в `session.json` полем `planner` и печатается в TUI.

`BUILD_UNAVAILABLE_REASON` **не меняется**: Build появится в 00h3, там же режим `opencode`
станет причиной отказа (`PlannerBypassedTransport`). Здесь только запись факта.

## files_allowed

- `crates/stellwerk-cli/src/plan.rs`
- `crates/stellwerk-cli/src/main.rs`
- `crates/stellwerk-cli/tests/plan.rs`

## Что делать

1. `pub enum PlannerMode { Host, Opencode }` + `#[derive(ValueEnum)]` и `as_str()`;
   `Commands::Plan { repo, #[arg(long, value_enum, default_value_t = PlannerMode::Host)] planner }`.
2. Две реализации одного поведения в `plan.rs`: текущая (opencode, без изменений) и новая
   `HostTransport`:
   - `PlannerHost` собирается **лениво, на первом `ask`**, и переиспользуется: повторный
     `ModelProxy::bind` на существующем пути — `EADDRINUSE`, а не «новый сокет»;
   - ошибка сборки (нет токена, плохой адрес) → `TransportEvent::Failed` с текстом, TUI
     остаётся живым: человек видит причину, а не падение;
   - ход идёт в отдельном потоке, результат — `Text` одним куском, затем `Finished`,
     ошибка — `Failed`;
   - `cancel()` **не прерывает** уже начатый ход (поток вправе доработать до `TURN_TIMEOUT`);
     приёмник при этом отбрасывается, и поздний ответ никого не обновит. Это записывается
     честно в документации метода, а не выдаётся за отмену.
3. Константы адреса и модели в `plan.rs`: `HOST_BASE_URL = "https://api.neuraldeep.ru/v1"`,
   `HOST_MODEL_ID = "qwen3.6-unlim-xl-noreason"` (голый id; префикс `neuraldeep/` — маршрут
   Pi, не поле API).
4. `PlanSession.planner: Option<String>` с `#[serde(default)]`: отсутствие поля в старых
   сессиях — «источник не записан», и это состояние не равно ни `host`, ни `opencode`.
5. `run_plan_tui(repo, mode)`: после `load_session`/`new` — если `planner` не записан,
   записать текущий режим; если записан другой — **не перетирать молча**: принять
   записанный и показать его, иначе утверждённый план сменил бы происхождение при
   перезапуске.
6. В `render_plan` — строка «ход модели: …» с режимом; для `opencode` явно «вне транспорта».

## Тесты (`tests/plan.rs`), 4 новых

- `the_session_records_the_planner_mode` — `host` попадает в `session.json`;
- `a_recorded_mode_is_not_overwritten_by_a_different_flag` — сессия с `host`, запуск с
  `--planner opencode` → в файле остаётся `host`;
- `an_old_session_without_the_field_is_not_treated_as_host` — без поля → `None`, а не
  `Some("host")`;
- `render_names_the_mode_and_calls_opencode_external` — в тексте TUI есть режим и пометка
  «вне транспорта».

Существующие тесты рендера и сессии должны остаться зелёными (поле опциональное).

## Приёмка

```
cargo test -p stellwerk-cli --test plan
cargo test -p stellwerk-cli
cargo clippy -p stellwerk-cli --all-targets --no-deps -- -D warnings
cargo fmt -p stellwerk-cli -- --check
```

C2: `src` ≤ 200 логических строк, `tests` ≤ 250, 3 файла.

## Запрещено

Менять `host_model.rs`, `planner_host.rs`, `BUILD_UNAVAILABLE_REASON`; удалять режим
`opencode`; молча подставлять режим при чтении сессии; трогать `stellwerk-task/src/cli.rs`.
