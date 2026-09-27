# packet-07
plan_id: M1
goal: Зафиксировать RED-базлайн хранилища: 20 из 21 тестов падают на `not implemented`, и ровно один остаётся зелёным.
files_allowed: [crates/stellwerk-store/tests/baseline_red.md]
max_diff_lines: 200
test: cargo test -p stellwerk-store
acceptance: Вывод показывает 20 failed и 1 passed; каждый panic содержит `not implemented: SqliteStore::`; зелёный ровно `task_crud::open_applies_migrations_without_error`.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [—]
data_class: public
lane: m1/store-red

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

Пакет не пишет код — фиксирует базлайн.

1. `cargo test -p stellwerk-store 2>&1 | tee /tmp/m1-store-red.txt`.
2. Проверить: `task_crud` 1/6, `event_append` 0/4, `module_store` 0/7, `concurrent` 0/3.
3. Проверить, что причина каждого падения — `not implemented: SqliteStore::…`, а не
   паника внутри теста или ошибка компиляции.
4. Сводку — в `crates/stellwerk-store/tests/baseline_red.md`.
5. Не приступать к реализации: это packet-08…12.

## Evidence

- `cargo test -p stellwerk-store` → сводка 20 failed / 1 passed.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
