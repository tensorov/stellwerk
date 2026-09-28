# packet-05a
plan_id: M2
goal: RED-спецификация трейта хранилища: прочитать фактический статус задачи и записать статус УСЛОВНО, без гонки между чтением и записью.
files_allowed: [crates/stellwerk-store/src/traits.rs, crates/stellwerk-store/src/sqlite.rs, crates/stellwerk-store/tests/status_transition.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-store --test status_transition
acceptance: RED: `bash -c 'cargo build -p stellwerk-store && cargo test -p stellwerk-store --test status_transition --no-run && ! cargo test -p stellwerk-store --test status_transition'` → exit 0.
forbidden: [cargo add, cargo install, правка contract.rs, расширение TaskSpec, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [04, M1-08, M1-09, M1-10, M1-11, M1-12]
data_class: public
lane: m2/store-contract-red

## Контекст

- **Зачем два метода, а не один.** Статус задачи лежит в колонке `tasks.status`, но
  прочитать статус ОДНОЙ задачи нечем: `TaskSpec` поля статуса не имеет, а
  `TaskStore` даёт `update_status` и `list_by_status` — то есть перебор 12 вариантов
  статуса с построчным `SELECT`.
- **Почему одного чтения мало.** Наивная цепочка
  `status_of → can_transition_to → update_status` оставляет промежуток, в который
  задачу может изменить другой исполнитель. Отдельные вызовы не становятся одной
  транзакцией автоматически: SQLite вправе завершить автоматически начатую
  транзакцию после каждого SQL-оператора. Читать реальное состояние и затем писать
  безусловно — значит потерять его ровно на записи.
- **Отсюда условная запись (CAS).** Проверка обязана выполняться ВНУТРИ операции
  обновления, а SQLite сериализует записи, поэтому гонки не остаётся.

## Что сделать

1. `traits.rs`: объявить в `TaskStore` ДВА метода, рядом с существующим
   `update_status`, сохранив стиль трейта (асинхронность, `Send`, типы ошибок):

   ```
   fn status_of(&self, id: &str) -> impl Future<Output = Result<Option<TaskStatus>, StoreError>> + Send;
   fn compare_and_set_status(&self, id: &str, expected: TaskStatus, next: TaskStatus)
       -> impl Future<Output = Result<bool, StoreError>> + Send;
   ```

   Контракт:
   - `status_of`: существующая задача → `Some(фактический статус)`; отсутствующая →
     `Ok(None)`; **нечитаемое значение статуса → ошибка, НЕ `None` и НЕ `Draft`**;
   - `compare_and_set_status`: `true` — обновлена ровно одна задача; `false` —
     ожидаемое состояние не совпало либо задачи нет; ошибка — проблема хранилища.

2. `sqlite.rs`: объявить обе реализации с телом `unimplemented!()` и приватный
   разбор строки в статус:

   ```
   fn str_to_status(s: &str) -> Result<TaskStatus, StoreError>
   ```

   Обратная сторона существующей `status_to_str`. Статусы в БД — PascalCase-строки
   serde (`Draft`, `Running`). Неизвестная строка — ошибка, а не `Draft`.

3. `tests/status_transition.rs` — четыре сценария, все на временной БД, без `sleep`
   и без надежды на порядок потоков:
   - `status_of_returns_actual_stored_status` — задача в `Running`, читаем `Some(Running)`;
   - `status_of_unknown_id_is_none_not_default` — `Ok(None)`, и явно НЕ `Draft`;
   - `compare_and_set_fails_when_expected_state_is_stale` — задача в `Draft`, CAS
     с `expected = Running` → `false`, а статус остался `Draft`;
   - `exactly_one_of_two_compare_and_set_from_same_state_wins` — два CAS из одного
     `expected` в разные цели, ровно один `true`.

4. Юнит-тест парсера в `sqlite.rs` (`#[cfg(test)] mod tests`): `str_to_status` на
   неизвестной строке даёт `Err`, а не `Ok(Draft)`. Проверка «битой строки в БД»
   делается здесь, а не интеграционным тестом: чтобы записать мусор в колонку,
   нужен прямой SQL, а это либо недоступный в интеграционном тесте крейд, либо
   `rusqlite` в dev-зависимостях, а это `cargo add` — запрещено.

5. Уже существующий `update_status` НЕ удалять и НЕ переводить на CAS. Он остаётся
   для прямых записей; переходы в lifecycle не будут его использовать.

## Evidence

- RED-вывод `cargo test -p stellwerk-store --test status_transition`: 4 failed
  (тесты компилируются и падают на заглушке, а не отказываются собираться).
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
