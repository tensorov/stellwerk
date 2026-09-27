# packet-09
plan_id: M2
goal: RED: запуск воркера и разбор потока — `runner` превращает JSON-события Pi в доменные события федерации.
files_allowed: [crates/stellwerk-task/src/runner.rs, crates/stellwerk-task/tests/runner.rs, crates/stellwerk-task/src/lib.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-task --test runner
acceptance: RED: пакет собирается, тест компилируется, прогон падает. Причина падения — заглушка, а не компиляция.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [08]
data_class: public
lane: m2/runner-red

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

1. `src/runner.rs`:
   - `pub struct RunRequest { spec: TaskSpec, model: ModelBinding, sandbox: Sandbox }`;
   - `pub struct RunOutcome { stop_reason: StopReason, tool_steps: u32, turns: u32,
     total_input_tokens: u64, verdict: Verdict }`;
   - `pub enum StopReason { Stop, Error, Timeout, Aborted }`;
   - `pub enum Verdict { Valid, InvalidLoop, InvalidNoToolSteps, InvalidTooFewTurns }`.
2. Инварианты verdict — те же пороги, что в `scripts/worker/dispatch.conf`:
   `GATE_REQUIRE_STOP_OK`, `GATE_REQUIRE_TOOL_STEPS`, `GATE_REQUIRE_TURNS`.
   Значения по умолчанию читай из переменных окружения с теми же именами, что и
   в `dispatch.conf` (`GATE_REQUIRE_TOOL_STEPS` и т.д.), чтобы пороги не расходились.
3. Тесты в `tests/runner.rs` используют ПОДДЕЛЬНЫЙ исполнитель (не Pi!):
   - `verdict_for(stop=Stop, steps=1, turns=2)` → Valid;
   - `verdict_for(stop=Error, …)` → Invalid (не задан stop ok);
   - `steps = 0` → Invalid (значит модель не использовала инструменты);
   - `turns = 1` → Invalid (подозрительно короткий прогон);
   - разбор строки JSON-события → `tool_steps` увеличивается на каждом `turn_end`.
4. `src/lib.rs`: добавить `pub mod runner;`.

## Evidence

- RED-вывод `cargo test -p stellwerk-task --test runner`.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
