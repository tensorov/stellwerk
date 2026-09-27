# packet-06
plan_id: M4
goal: GREEN — цепочка evidence `ModelClaim → ToolAttestation → Validation → GateResult` для каждого вызова.
files_allowed: [crates/stellwerk-task/src/evidence.rs, crates/stellwerk-task/tests/vertical.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-task --test vertical
acceptance: `cargo test -p stellwerk-task --test vertical` → 0 failed.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [05]
data_class: public
lane: m4/vertical

## Контекст

- `EvidenceKind` уже есть в `stellwerk-core/src/contract.rs` — `ModelClaim`,
  `ToolAttestation`, `Validation`, `GateResult`. **Своих видов не заводить.**
- Политика, которая здесь закрепляется: `ModelClaim` может прийти только от
  модели, `ToolAttestation` — только от запущенного инструмента. Раннер не имеет
  права дописать `ToolAttestation` за инструмент, который не запускался: при
  `Deny` worker не вызван, значит и аттестации быть не может. **Это тот случай,
  где «отсутствие данных — это отдельное состояние»:** нет запуска — значит
  `ToolAttestation` не создаётся, а не создаётся пустым.
- В чеках — **только хеши и метаданные**, никогда тела вывода и никогда секреты.
  Это инвариант из индекса M3, и он переносится сюда без исключений.

## Что сделать

1. `src/evidence.rs`:
   - `pub fn record_call(store: &dyn EventStore, call: &Call, worker_ran: bool, outcome: &Verdict) -> Result<Vec<EvidenceKind>, RunError>`.
   - `worker_ran == true` → цепочка `ModelClaim → ToolAttestation` (+ `Validation`,
     если `Verdict` требует) и `GateResult`.
   - `worker_ran == false` → **только** `ModelClaim` и `GateResult`. `ToolAttestation`
     не создаётся, и это должно быть видно по отсутствию элемента, а не по
     заглушке.
   - Полей тела вывода и секретов в структуре записи нет вообще — не пустая
     строка, а отсутствующее поле.
2. Никаких `unwrap`/`expect` в путях, достигаемых из `run_once`. Ошибка
   хранилища — это ошибка, а не повод объявить успех.
3. `tests/vertical.rs` **не трогать** — пакет реализует, чтобы уже написанные
   RED-тесты прошли. Единственное допустимое дополнение — новый сценарий, если
   без него тест невыразим; и тогда он пишется **сюда**, а не правкой чужого.

## Evidence

- `cargo test -p stellwerk-task --test vertical` → 0 failed.
- clippy `-D warnings` и `fmt --check` чисто.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
