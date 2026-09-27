# packet-05
plan_id: M2
goal: RED: продвижение статусов задачи через предикаты ядра — запрещённый переход обязан быть отвергнут.
files_allowed: [crates/stellwerk-task/src/lifecycle.rs, crates/stellwerk-task/tests/lifecycle.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-task --test lifecycle
acceptance: RED: `bash -c 'cargo build -p stellwerk-task && cargo test -p stellwerk-task --test lifecycle --no-run && ! cargo test -p stellwerk-task --test lifecycle'` → exit 0.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [04]
data_class: public
lane: m2/lifecycle-red

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

## Что сделать

1. `src/lifecycle.rs`: функция
   `pub fn advance(store: &dyn TaskStore, id: &str, next: TaskStatus) -> Result<TaskStatus, LifecycleError>`.
   Проверка перехода — ТОЛЬКО через `TaskStatus::can_transition_to` из контракта.
   Свой свод переходов заводить запрещено.
2. `LifecycleError` (thiserror): `Store(#[from] StoreError)`, `IllegalTransition { from, to }`,
   `Blocked(String)`.
3. Тесты в `tests/lifecycle.rs`:
   - `Draft → Planned` разрешён;
   - `Draft → Verified` запрещён (прыжок без прогона);
   - `Running → Suspended` разрешён, `Suspended → Running` разрешён;
   - `Done → Running` запрещён;
   - `Verified → Validated` разрешён, `Validated → Running` запрещён;
   - успешный переход обновляет статус в хранилище (проверяется `list_by_status`);
   - запрещённый переход НЕ меняет статус в хранилище.
   Первые тесты зелёные сразу (нужны только предикаты M1), последние падают —
   `advance` ещё заглушка.
4. Не реализуй `advance` — это packet-06.

## Evidence

- RED-вывод `cargo test -p stellwerk-task --test lifecycle`.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
