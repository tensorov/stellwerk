# packet-03
plan_id: M2
goal: RED-скелет CLI `task apply|get|watch|cancel|logs` на clap: парсинг работает, run() — заглушка.
files_allowed: [crates/stellwerk-task/src/cli.rs, crates/stellwerk-task/src/main.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-task --lib cli
acceptance: RED: `bash -c 'cargo build -p stellwerk-task && cargo test -p stellwerk-task --lib cli --no-run && ! cargo test -p stellwerk-task --lib cli'` → exit 0 (собирается, компилируется, падает на assert).
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [02]
data_class: public
lane: m2/cli-red

## Контекст

- **Новый крейт `stellwerk-task`.** Ничего из `stellwerk-core` и `stellwerk-store`
  не редактируется: крейт только вызывает их публичные API.
- Крейт зависит от `stellwerk-core` (контракт) и `stellwerk-store` (трейты) по path.
  Зависимости — только из `[workspace.dependencies]`. `cargo add` запрещён.
- Команда kubectl-shaped: `stellwerk task apply|get|watch|cancel|logs`.
  Один глагол — одно действие; комбо-команд нет.
- Задача описывается TOML-файлом (`toml` уже в workspace) — YAML-крейт не нужен.
- Статусы ведутся ЧЕРЕЗ предикаты ядра (`TaskStatus::can_transition_to`). Не заводи
  собственный свод переходов: единственный источник истины — контракт.
- Уже закрытые пакеты M1 (packet-02…12) — их предикаты и трейты считай готовыми.

## Что сделать

1. `cli.rs`: clap-derive. Структура `Cli { task: TaskCommands }`, подкоманда `Task`
   с вариантами:
   - `apply --file <PATH>` — применить спецификацию;
   - `get <ID>` — прочитать спецификацию и статус;
   - `watch <ID>` — поток событий;
   - `cancel <ID>` — остановить (перевод в Suspended);
   - `logs <ID>` — журнал прогонов.
2. `main.rs`: `fn main()` разбирает аргументы и вызывает `cli::run(cli) -> i32`
   (0 — успех, 1 — ошибка исполнения, 2 — ошибка разбора). Код возврата через
   `std::process::exit` — иначе статус потеряется.
3. RED-тесты в `src/cli.rs` (`#[cfg(test)] mod tests`, путь содержит `cli`):
   - `Cli::try_parse_from(["stellwerk","task","apply","--file","t.toml"])` → Ok;
   - `… "get" "t-1"` → Ok;
   - `… "watch" "t-1"`, `"cancel" "t-1"`, `"logs" "t-1"` → Ok;
   - `… "nonsense"` → Err (неизвестная подкоманда);
   - `… "get"` без ID → Err;
   - `run(cli)` на заглушке возвращает 1 — тест на это и падает.
4. Точное имя бинаря в тестах первой строкой: `stellwerk`.

## Evidence

- RED-вывод `cargo test -p stellwerk-task --lib cli`.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
