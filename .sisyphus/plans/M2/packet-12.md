# packet-12
plan_id: M2
goal: Реализовать запись событий и цепочку evidence: ModelClaim → ToolAttestation → Validation → GateResult.
files_allowed: [crates/stellwerk-task/src/events.rs, crates/stellwerk-task/src/evidence.rs, crates/stellwerk-task/src/lib.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-task
acceptance: `cargo test -p stellwerk-task` → все тесты крейта зелёные. Прогон `apply` на temp-базе печатает `applied <id>` и завершается 0.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [11]
data_class: public
lane: m2/evidence

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

1. `src/events.rs`: реализовать запись (в packet-11 было RED).
2. `src/evidence.rs` — сердце закона. Правила из `AGENTS.md`:
   - `EvidenceKind::ModelClaim` пишет ТОЛЬКО модель; команда в нём пустая;
   - `EvidenceKind::ToolAttestation` — результат запуска инструмента; именно он делает
     запись доказательством;
   - `EvidenceKind::Validation` — итог gate;
   - `GateResult.chain_tail_hash` обязан меняться при каждой новой записи цепочки.
3. Хеш: `sha2` уже в workspace. Хвост цепочки = хеш от хвоста + новая запись
   (детерминированно, без времени внутри, иначе тесты не повторяемы).
4. `GateResult.passed == true` ТОЛЬКО если: есть хотя бы один ToolAttestation, для него
   есть Validation, и `RunOutcome.verdict == Valid`. Иначе `passed=false`, а в `missing`
   — человекочитаемые причины. Gate, который проходит при невалидном прогоне, хуже
   отсутствия gate: он даёт ложную уверенность.
5. Статус задачи двигается только через `lifecycle::advance` (packet-06), по шагам
   `Delegated → Running → EvidenceSubmitted → Verified → Validated → Done`, а при
   невалидном вердикте — в `Failed` с записью `run.invalid`.
6. `src/lib.rs`: `pub mod evidence;` и `pub mod events;`.
7. Сетевых вызовов к PolyForge здесь нет: gate считается локально, цепочка пишется
   в `evidence`-таблицу через отдельный трейт в будущем M3.

## Evidence

- `cargo test -p stellwerk-task` → passed.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
