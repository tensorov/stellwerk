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
- `pub mod lifecycle;` уже объявлен в `src/lib.rs` пакетом 05. Здесь `lib.rs` не
  трогается и не упоминается: он вне `files_allowed`.

## Что сделать

Трехшаговая последовательность. **Читать, потом проверять, потом писать условно** —
порядок обязателен, и запись безусловная запрещена.

1. **Прочитать фактический статус.** `store.status_of(id)`:
   - `Ok(None)` → задачи нет, пробрасывается `StoreError::NotFound`;
   - `Err(e)` → пробрасывается как есть;
   - `Ok(Some(current))` → единственный источник истины о текущем состоянии.
   `store.get(id)` НЕ подходит: `TaskSpec` поля статуса не имеет, и статус из него
   не извлечь. Никакого `from` от вызывающего не существует и не появится.
2. **Проверить переход.** `current.can_transition_to(next)`; иначе
   `IllegalTransition { from: current, to: next }` и **ничего не записывать**.
3. **Записать условно.** `store.compare_and_set_status(id, current, next)`:
   - `true` → успех;
   - `false` → `StateConflict { expected: current, next }`. **Повторную безусловную
     запись делать нельзя**: это ровно та потеря проверенного состояния, ради которой
     чтение и запись разделены. Возврат `StateConflict` — не откат, а отказ.
   Между шагами 1 и 3 задачу мог сменить другой исполнитель; поэтому шаг 3 несёт
   условие по статусу, а не полагается на результат шага 1.
4. `next` == `TaskStatus::Blocked` → дополнительно вернуть `Blocked(…)` с причиной
   **после успешной записи статуса** (причина — часть вызывающего кода, не трейта).
   Это единственный случай, когда функция возвращает не `TaskStatus`; вызывающий
   обязан понимать, что статус при этом записан.
5. Успех: вернуть `next`.

`update_status` для переходов не используется — он безусловный и не проверяет
состояние. Он остаётся в трейте для прямых записей.

**Сигнатура, к которой сводится реализация** (проверено в packet-05, отклонение
зафиксировано в коде `lifecycle.rs`):

```
pub fn advance(store: &impl TaskStore, id: &str, next: TaskStatus) -> Result<TaskStatus, LifecycleError>
```

Параметр — `&impl TaskStore`, а НЕ `&dyn TaskStore`. Трейт возвращает
`impl Future + Send` (RPITIT) и потому не dyn-совместим: `&dyn TaskStore` не
компилируется, `error[E0038]`. Починить это можно только правкой трейта в
закрытом крейте store, что вне `files_allowed` этого пакета. Аргументов по-прежнему
три, `from` не появился.

`advance` синхронна при async-хранилище: она блокирует поток на собственном
рантайме. Поэтому её **нельзя вызывать изнутри активного рантайма** — в частности,
тест не должен быть `#[tokio::test]`; в RED-пакете он и не такой, а его сборка
идёт собственным рантаймом.

## Evidence

- `cargo test -p stellwerk-task --test lifecycle` → 7 passed.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
