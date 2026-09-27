# packet-09
plan_id: M1
goal: Реализовать `TaskStore::update_status` и `TaskStore::list_by_status`.
files_allowed: [crates/stellwerk-store/src/sqlite.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-store --test task_crud
acceptance: `cargo test -p stellwerk-store --test task_crud` → 5 passed; 1 failed (`create_twice_does_not_silently_overwrite`, если он уже закрыт в packet-08 — тогда 6 passed и это ошибка разметки: сообщить, а не молчать).
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [08]
data_class: public
lane: m1/store-task-status

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

1. `update_status`: `UPDATE tasks SET status = ?2, updated_at = … WHERE id = ?1`.
   Затронуто 0 строк → `StoreError::NotFound`.
2. `list_by_status`: `SELECT data FROM tasks WHERE status = ?1 ORDER BY id`
   (используется индекс `idx_tasks_status`). Детерминированный порядок важнее
   «естественного» порядка SQLite.
3. Строка статуса — ровно та, что даёт serde: `TaskStatus` сериализуется в PascalCase
   (`Draft`, `Running`). НЕ переводи в snake_case: иначе после перезапуска базы
   `list_by_status` перестанет находить задачи.
4. `created_at` при обновлении не меняется.

## Evidence

- `cargo test -p stellwerk-store --test task_crud` → 5 passed.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
