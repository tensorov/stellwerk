# packet-06
plan_id: M2
goal: Реализовать продвижение статусов: `advance` обновляет хранилище и отвергает нелегальные переходы.
files_allowed: [crates/stellwerk-task/src/lifecycle.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-task --test lifecycle
acceptance: `cargo test -p stellwerk-task --test lifecycle` → 7 passed; 0 failed.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [05]
data_class: public
lane: m2/lifecycle

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

1. `advance`: `store.get(id)`; `None` → `StoreError::NotFound` пробрасывается.
2. Определить текущий статус. Учти: `TaskSpec` НЕ несёт статус (это решение фазы 4) —
   статус лежит в колонке таблицы. Значит нужен способ его прочитать:
   добавь в `SqliteStore` **не** новый трейт-метод, а прочитай через
   `list_by_status` там, где статус известен, либо расширь трейт МИНИМАЛЬНО:
   если расширяешь `TaskStore` — это правка `stellwerk-store`, а значит ты вылезаешь за
   files_allowed. **Правильный путь:** держи текущий статус в памяти вызывающего кода
   (CLI его знает) и передавай в `advance` явно: `advance(store, id, from, to)`.
   Перечитай этот пункт перед кодом — иначе упрёшься в отсутствующий метод чтения статуса.
3. Проверить `from.can_transition_to(to)`; иначе `IllegalTransition { from, to }` и
   **не** вызывать `update_status`.
4. `to` == `TaskStatus::Blocked` → дополнительно вернуть `Blocked(…)` с причиной
   после успешной записи статуса (причина — часть вызывающего кода, не трейта).
5. Успех: `store.update_status(id, to)` и возврат `to`.
6. Добавь `pub mod lifecycle;` в `src/lib.rs` одной строкой, ПОСЛЕ существующих `pub mod`.

## Evidence

- `cargo test -p stellwerk-task --test lifecycle` → 7 passed.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
