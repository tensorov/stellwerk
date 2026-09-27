# packet-08
plan_id: M1
goal: Реализовать `TaskStore::create` и `TaskStore::get`; повторный create обязан давать Conflict, а не тихую перезапись.
files_allowed: [crates/stellwerk-store/src/sqlite.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-store --test task_crud
acceptance: `cargo test -p stellwerk-store --test task_crud` → минимум 2 passed (`create_then_get_returns_the_same_spec`, `get_unknown_id_is_none_not_error`); остальные task_crud ещё красные — они в packet-09/10.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [07]
data_class: public
lane: m1/store-task-crud

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

1. `create`: сериализовать `TaskSpec` в колонку `data`; статус новой задачи —
   `TaskStatus::Draft`; `workspace_id = spec.workspace_ref`; записать `created_at`
   и `updated_at` в RFC 3339 UTC.
2. Повторный `id` → `StoreError::Conflict`. Различай нарушение уникальности по коду
   rusqlite (`SqliteFailure` с `ConstraintViolation`), а не по тексту сообщения.
3. `get`: `SELECT data FROM tasks WHERE id = ?1`; отсутствие → `Ok(None)` (не ошибка).
4. `update_status`, `list_by_status`, `list_by_workspace` не трогать.

## Evidence

- `cargo test -p stellwerk-store --test task_crud create_then_get` → passed.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
