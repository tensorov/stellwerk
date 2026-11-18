# N1-01 — вертикальный срез: процесс в bwrap пишет receipt

**label:** — · **mutation_gate:** no (создание крейта + CLI-обвязка)
**depends_on:** [] · **ADR:** не требуется

## Цель

`stellwerk run <task.toml>` существует и делает минимальный настоящий проход:
читает task.toml, стартует процесс **внутри bwrap** с `env_clear`, вызывает
`run_once`, пишет receipt в SQLite, печатает вердикт.

Движок — **захардкоженный `/bin/echo`** (решение 4: это smoke процесса,
не proof агентного успеха). Настоящий движок и разбор Pi-потока — N1-02/N1-05.

## production_call

- `runner::run_once` ← `stellwerk_cli::run::execute`
- `m5::sandbox_bwrap::Sandbox::command` ← `stellwerk_cli::run::EchoWorker::exec`

## files_allowed

- `Cargo.toml` — добавить `crates/stellwerk-cli` в members
- `crates/stellwerk-cli/Cargo.toml`
- `crates/stellwerk-cli/src/main.rs`
- `crates/stellwerk-cli/tests/run.rs`
- `crates/stellwerk-cli/examples/n1-smoke.toml`

**limit_waiver:** `files: 5` — создание крейта требует ровно этих файлов;
разбиение без потери TDD-теста и рабочего `task.toml` невозможно.

**корневой `Cargo.toml` — `contract-change` по AGENTS.md; пакет помечен.**

## Что делать

1. Крейт зависит от `stellwerk-task`, `stellwerk-store`, `stellwerk-core`,
   `stellwerk-gateway`, `clap`, `tokio`, `serde` (всё из workspace).
2. Единственная подкоманда: `run <path: PathBuf>`.
3. `examples/n1-smoke.toml` — **валидная** фикстура. Внимание: `validate()`
   отклоняет нули, поэтому `limits` обязан быть заполнен **явно**
   (`max_files`, `max_diff_lines`, `timeout_seconds` > 0). Дефолт
   `TaskFileLimits::default()` даёт нули и фикстуру не проходит.
4. Собрать `Sandbox { work, tmp, read_only, window }`: `work` — временный каталог
   прогона, `tmp` — **отдельный** (общий `/tmp` у двух прогонов — гонка),
   `window` — из `limits.timeout_seconds`.
5. `EchoWorker` реализует `Worker`:
   - `exec` строит `Sandbox::command(work, /bin/echo, &[goal], budget)`;
   - **сразу** `.env_clear()` на возвращённом `Command` — это и есть требование
     N1, и `m5/*` не трогается: `command()` уже вернул `Command`;
   - `spawned()` считает фактические запуски, включая упавшие.
6. `run_once` с `SqliteStore::open(&db)`.
7. Печать `verdict=<Allow|Deny|Suspended|Ask>`, `receipt=<id>`; код 0.

**Печатать `gate`/`PASS`/`Done` запрещено** — решение 5: до `Validation`
результат ждёт человека.

## До начала: проверить bwrap

```sh
bwrap --unshare-user --ro-bind / / true; echo "BWRAP_EXIT=$?"
```

**Критерий — код возврата, а не вывод.** `true` по определению молчит, поэтому
успешная проверка даёт **пустой вывод и `BWRAP_EXIT=0`**. Смотрить на вывод
бессмысленно: успех тихий.

- `BWRAP_EXIT=0` → можно продолжать;
- иначе → **BLOCKED** с приведённым кодом и stderr. Обход userns не искать:
  тихий обход сделает приёмку зелёной при неработающей изоляции.

## Запрещено трогать

`justfile`, `.github/`, `scripts/plan-lint.py`, `xtask/src/main.rs`,
`crates/stellwerk-task/src/m5/**`, `crates/stellwerk-gateway/**`,
`crates/stellwerk-task/src/evidence.rs`, `runner.rs`, store.

## Приёмка (владелец запускает сам)

```sh
cargo build --workspace --all-targets; echo "BUILD_EXIT=$?"
cargo test -p stellwerk-cli; echo "TEST_EXIT=$?"
cargo run -p stellwerk-cli -- run crates/stellwerk-cli/examples/n1-smoke.toml; echo "RUN_EXIT=$?"
```

**BUILD_EXIT=0 · TEST_EXIT=0 · RUN_EXIT=0.** Ошибка сборки, «0 тестов» и
`--no-run` — FAIL (решение 7).

**Пустой вывод `run` = провал**, даже при RUN_EXIT=0: без `verdict=` и
`receipt=` приёмка не засчитана.

Grep-проверка, что вызов не только в тестах:

```sh
grep -rn "run_once" crates/stellwerk-cli/src/
```