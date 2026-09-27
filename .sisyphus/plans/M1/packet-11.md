# packet-11
plan_id: M1
goal: Реализовать `EventStore::append`, `list_since` и `list_by_task` — поток событий должен пережить конкурентную запись.
files_allowed: [crates/stellwerk-store/src/sqlite.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-store --test event_append
acceptance: `cargo test -p stellwerk-store --test event_append` → 4 passed; 0 failed. Затем `cargo test -p stellwerk-store --test concurrent` → 3 passed — это проверка потокобезопасности, она обязана позеленеть вместе с `append`.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [10]
data_class: public
lane: m1/store-events

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

Самая ответственная реализация milestone: потерянное событие ломает и `watch`, и gate.

1. `append`: `INSERT INTO events (id, task_id, ts, created_at) VALUES (?, ?, ?, ?)`.
   `task_id` берётся из `event.payload["task_id"]` — колонка денормализована ради
   индекса, это осознанное решение из `0001_init.sql`.
2. Повторный `id` → `StoreError::Conflict`: поток append-only, дубль означает баг.
3. `list_since(ts)`: `WHERE ts >= ?1 ORDER BY ts, id`. Граница ВКЛЮЧИТЕЛЬНАЯ: тест ждёт
   первым `evt-0050`.
4. `list_by_task`: `WHERE task_id = ?1 ORDER BY ts, id` (`idx_events_task`).
5. **Потокобезопасность:** не держи `MutexGuard` через `.await`. Схема: собрать результат
   внутри блока `{ let guard = self.conn.lock()…; … }`, отпустить guard, затем await.
6. Если понадобится payload события — добавляй колонку миграцией `0003`, а не правкой
   `0001`: применённые миграции не переписываются.

## Evidence

- `cargo test -p stellwerk-store --test event_append` → 4 passed; `--test concurrent` → 3 passed.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
