# packet-05b
plan_id: M2
goal: Реализовать чтение фактического статуса задачи и условную запись статуса в `SqliteStore`.
files_allowed: [crates/stellwerk-store/src/traits.rs, crates/stellwerk-store/src/sqlite.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-store --test status_transition
acceptance: `cargo test -p stellwerk-store --test status_transition` → 4 passed; 0 failed.
forbidden: [cargo add, cargo install, правка тестов, правка contract.rs, расширение TaskSpec, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [05a, M1-08, M1-09, M1-10, M1-11, M1-12]
data_class: public
lane: m2/store-contract

## Контекст

- `SqliteStore` держит `Mutex<Connection>`; держи `MutexGuard` НЕ через `.await`.
- Статусы в БД — PascalCase-строки serde (`Draft`, `Running`). Не переводи в snake_case.
- Сигнатуры уже объявлены в packet-05a с телом `unimplemented!()`. Здесь меняется
  ТОЛЬКО тело; объявлять новые методы или менять подписи нельзя.
- Существующий `update_status` остаётся как есть и не переводится на CAS.

## Что сделать

1. `str_to_status(s: &str) -> Result<TaskStatus, StoreError>`: полный `match` по всем
   двенадцати вариантам `TaskStatus`, как в обратную сторону от `status_to_str`.
   Неизвестная строка — `Err(StoreError::…)`. **Не** `Ok(Draft)`, **не** `unwrap_or`.

2. `status_of`: `query_row` по `WHERE id = ?1` с `optional()`.
   - строка есть → `Ok(Some(str_to_status(значение)?))`;
   - строки нет → `Ok(None)`;
   - значение нечитаемо → ошибка от `str_to_status` пробрасывается как есть.

3. `compare_and_set_status(id, expected, next)`:

   ```sql
   UPDATE tasks SET status = ?3, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
   WHERE id = ?1 AND status = ?2
   ```

   - вернул 1 → `Ok(true)`; вернул 0 → `Ok(false)`; ошибка → пробрасывается.
   - **Ноль обновлённых строк НЕ является SQL-ошибкой в SQLite.** Результат
     обязательно проверяется в коде: `Ok(0)` и «ничего не записалось» — разные вещи,
     и молчаливый `Ok(true)` здесь означал бы потерю чужого состояния.
   - `Ok(false)` означает и «статус не совпал», и «задачи нет». Различать эти два
     случая пакет не требует: вызывающий обязан трактовать и то и другое как отказ
     перехода, а не как успех.

4. Не трогать ничего, кроме тел этих трёх функций.

## Evidence

- `cargo test -p stellwerk-store --test status_transition` → 4 passed.
- Регрессия: `cargo test -p stellwerk-store` целиком.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
