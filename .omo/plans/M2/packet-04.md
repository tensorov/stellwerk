# packet-04
plan_id: M2
goal: Реализовать dispatch CLI: `apply` и `get` работают поверх хранилища; `watch|cancel|logs` — честные заглушки с внятным сообщением.
files_allowed: [crates/stellwerk-task/src/cli.rs, crates/stellwerk-task/src/main.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-task --lib cli
acceptance: `cargo test -p stellwerk-task --lib cli` → все тесты cli зелёные; `apply` на существующем файле печатает идентификатор задачи и завершается 0; `get` несуществующей задачи завершается 1.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [03]
data_class: public
lane: m2/cli-green

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

- `SqliteStore` держит `Mutex<Connection>`; держи `MutexGuard` НЕ через `.await`.
- Статусы в БД — PascalCase-строки serde (`Draft`, `Running`). Не переводи в snake_case.

## Что сделать

1. `run(cli) -> i32`. Путь к базе — переменная окружения `STELLWERK_DB`
   (значение по умолчанию — путь в `std::env::temp_dir()`), чтобы тесты не трогали
   рабочую базу. Ключи и секреты не читать.
2. `apply --file P`: `TaskFile::load` → `into_spec` → `TaskStore::create`.
   Печать `applied <id>`; при повторном id — код 1 и текст про конфликт.
3. `get <id>`: `TaskStore::get`; `None` → код 1, `Some` → печать id, goal, workspace_ref.
4. `watch|cancel|logs`: вернуть код 1 и stderr-сообщение
   `not implemented: <команда> (packet-NN)`. Заглушка обязана называть пакет, который
   её реализует: иначе следующий воркер её не найдёт.
5. `main.rs`: `std::process::exit(cli::run(Cli::parse()))`.
6. Никаких сетевых вызовов и никаких запусков Pi в этом пакете — это packet-07+.

## Evidence

- `cargo test -p stellwerk-task --lib cli` → passed; ручной прогон `apply`/`get` на temp-базе.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
