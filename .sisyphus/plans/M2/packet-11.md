# packet-11
plan_id: M2
goal: RED: запись событий прогона в хранилище — каждый шаг Pi обязан стать событием федерации.
files_allowed: [crates/stellwerk-task/src/runner.rs, crates/stellwerk-task/src/events.rs, crates/stellwerk-task/tests/events.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-task --test events
acceptance: RED: пакет собирается, тест компилируется, прогон падает на заглушке.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [10]
data_class: public
lane: m2/events-red

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

1. `src/events.rs`: маппинг доменных событий прогона в `Event` из контракта:
   - старт прогона → `task.delegated` / `run.start`, `RiskClass::R1`;
   - tool-call → `run.step`, `R1` (правка файла);
   - сетевой вызов → `run.step` с `RiskClass::R2` в payload;
   - стоп → `run.end`, риск по фактическому максимуму шагов;
   - невалидный вердикт → `run.invalid`, `R1`, payload с причиной (`loop`,
     `no_tool_steps`, `too_few_turns`).
   `EventSource::System`, `workspace` = `spec.workspace_ref`, `ts` — RFC 3339 UTC.
2. Обязательно: `payload` обязан содержать `task_id` — по нему `EventStore::list_by_task`
   делает выборку (это денормализованная колонка из packet-11 в M1).
3. Тесты в `tests/events.rs` (на поддельном прогоне, без Pi):
   - каждый tool-call даёт ровно одно событие `run.step`;
   - `run.invalid` появляется при `Verdict::InvalidLoop` и несёт причину в payload;
   - у всех событий `payload["task_id"] == spec.id`;
   - события пишутся в `SqliteStore` и читаются `list_by_task` в том же порядке.

## Evidence

- RED-вывод `cargo test -p stellwerk-task --test events`.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
