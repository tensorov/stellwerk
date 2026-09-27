# packet-10
plan_id: M1
goal: Реализовать `TaskStore::list_by_workspace` — весь `task_crud` становится зелёным.
files_allowed: [crates/stellwerk-store/src/sqlite.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-store --test task_crud list_by_workspace
acceptance: `cargo test -p stellwerk-store --test task_crud` → 6 passed; 0 failed (весь файл зелёный).
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [09]
data_class: public
lane: m1/store-task-ws

## Контекст

- Крейт `stellwerk-store`, реализация — `src/sqlite.rs`.
- `SqliteStore::open()` и применение миграций УЖЕ работают (зелёный тест
  `open_applies_migrations_without_error`). Миграции не переписывать: применённые
  миграции не меняются, новые — новым файлом.
- Методы трейтов объявлены как `fn … -> impl Future<Output = …> + Send`.
- `SqliteStore` держит `Mutex<Connection>`, поэтому НЕЛЬЗЯ удерживать `MutexGuard`
  через `.await`: сначала собрать результат внутри блока, отпустить guard, потом await.
  Иначе остальные потоки встанут.
- Время — RFC 3339 UTC. Новых зависимостей нет: бери `std::time::SystemTime`.
- RED-тесты написаны ночной сессией (фаза 4) в `crates/stellwerk-store/tests/`.

## Что сделать

1. `SELECT data FROM tasks WHERE workspace_id = ?1 ORDER BY id` (индекс
   `idx_tasks_workspace`).
2. Колонка `workspace_id` заполняется в `create` (packet-08). Проверь тестом; лишних
   правок не делай.
3. Порядок по `id` обязателен: результат должен быть воспроизводимым.

## Evidence

- `cargo test -p stellwerk-store --test task_crud` → 6 passed.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
