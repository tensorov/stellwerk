# packet-12
plan_id: M1
goal: Реализовать `ModuleStore`: `get`, `list`, `upsert_lock_entry` — весь крейт `stellwerk-store` становится GREEN.
files_allowed: [crates/stellwerk-store/src/sqlite.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-store --test module_store
acceptance: `cargo test -p stellwerk-store` → 21 passed; 0 failed. Ни один тест крейта не падает.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [11]
data_class: public
lane: m1/store-modules

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

1. `upsert_lock_entry`: `INSERT INTO modules (id, name, risk, data, created_at, updated_at)
   VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET name=excluded.name,
   risk=excluded.risk, data=excluded.data, updated_at=excluded.updated_at`.
   Идемпотентность по имени обязательна: `module measure` переписывает запись при каждом
   замере. `created_at` при обновлении НЕ трогай.
2. `get(name)`: `SELECT data FROM modules WHERE name = ?1` → `Ok(None)` при отсутствии.
3. `list()`: `ORDER BY name` (тест ждёт `alpha, mike, zebra`).
4. `data` — `serde_json::to_string(&ModuleRecord)`; `risk` — `to_string()` либо NULL.
5. Идемпотентность обязана держаться и под гонкой (`upsert_twice_does_not_duplicate`).

## Evidence

- `cargo test -p stellwerk-store` → 21 passed. Это закрывает M1 по хранилищу.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
