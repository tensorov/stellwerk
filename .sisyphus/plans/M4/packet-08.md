# packet-08
plan_id: M4
goal: GREEN — один Task проходит все стадии до `Done` с верифицированным `ToolAttestation` и закрытым gate.
files_allowed: [crates/stellwerk-task/tests/vertical.rs, crates/stellwerk-task/src/lifecycle.rs, crates/stellwerk-task/src/runner.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-task --test vertical
acceptance: `cargo test -p stellwerk-task --test vertical` → 0 failed, включая сценарий полного прохода. PolyForge: `evidence_verify` на команде этого пакета → exit 0, `gate_evaluate` passed.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [04, 06, 07, M2-05, M2-06]
data_class: public
lane: m4/vertical

## Контекст

- Финальный пакет M4 и его **предикат приёмки**: не «все тесты зелёные», а «один
  Task дошёл до `Done` с верифицированной `ToolAttestation` и закрытым gate».
- Жизненный цикл задачи ведётся предикатами `TaskStatus::can_transition_to`.
  Переходы, которые обязан пройти вертикальный срез:
  `Draft → Planned → Reviewed → Delegated → Running → EvidenceSubmitted → Verified → Validated → Done`.
- `lifecycle.rs` появляется в M2-05/06. Этот пакет его использует, а не переписывает.

## Что сделать

1. `tests/vertical.rs` — **новый** сценарий `happy_path_reaches_done_with_verified_attestation`:
   - применить фикстуру через `TaskFile::load` (формат доказан читаемым: задача
     `m4-vertical-demo`, см. раздел «Фикстура» ниже);
   - `authorize` даёт `Allow` (инструмент классифицирован как `R0`, данные `Public`,
     вызывающий `L2Contributor`, интерактивно);
   - worker вызван ровно один раз;
   - чек **вызова** содержит `ModelClaim` и `ToolAttestation`, а `GateResult`
     закрывается на уровне **прогона** — как требует `goal` этого же пакета
     («все стадии до `Done` с… закрытым gate») и как устроен `evaluate`:
     `pub fn evaluate(outcome: &RunOutcome, chain: &[EvidenceEntry]) -> GateResult`
     берёт `RunOutcome` (`stop_reason`, `tool_steps`, `turns`, `total_input_tokens`,
     `verdict`), то есть исход прогона, а не вызова;

     `can_transition_to`;
   - статус дошёл до `Done` **через предикаты**, и каждый переход был разрешён
     `can_transition_to`;
   - `ToolAttestation` присутствует и **непустая**: у разрешённого вызова есть
     хеш результата, и это отличие от `Deny`-вызова, где аттестации нет вовсе.

     > **Правка 2026-09-29 (F-42).** Здесь стояло «чек содержит `ModelClaim`,
     > `ToolAttestation`, `GateResult`», то есть `GateResult` — в чеке **отдельного
     > вызова**. Это противоречило `goal` пакета и сигнатуре `evaluate` одновременно,
     > то есть было ложным утверждением пакета о самом себе, а не выбором между
     > прочтениями. Проверено: `RunOutcome` несёт `stop_reason`, `tool_steps`, `turns`,
     > `total_input_tokens`, `verdict` — это прогон. Класть прогонный артефакт в чек
     > каждого из вызовов означало бы, что чек врал бы о масштабе: вызовов было пять,
     > а «результат прогона» ехал в каждом. Закон 6 запрещает превращать несогласованное
     > в правдоподобное значение, и `GateResult` не измеряет вызов.
2. Негативный сосед: `denied_call_leaves_no_attestation` — уже есть в packet-05,
   здесь **проверь**, что он не сломался после появления happy path: happy path не
   имеет права создавать `ToolAttestation` для вызовов, которых не было.
3. Один прогон должен доказать и «прошло», и «что отказ не был проверен тем же
   кодом». Если сценарий не может это выразить, раздели на два теста — не
   ослабляй ни один.

## Фикстура — доказанная читаемость, а не предположение

`m4-vertical-demo` проверена прогоном настоящего `TaskFile::load` **до** написания
этих пакетов: `task apply --file <fixture>` вернул `applied m4-vertical-demo`
(exit 0), `task get` вернул `id`, `goal`, `workspace_ref`. Прошлая ночная смена
погорела ровно на непроверяемой фикстуре, поэтому здесь это факт, а не обещание.
Формат: `[task]` с `id`, `goal`, `workspace_ref`, `model_ref`, `packet` и
`[task.limits]` с шестью числами и `data_class = "internal"`. Парсер — `toml`
с `deny_unknown_fields`, то есть лишнее поле в фикстуре **сломает** загрузку.

## Почему пакет не «реализует M4 целиком»

Каждый предыдущий пакет уже ввёл свою поверхность и свои тесты. Этот пакет
связывает их и проверяет связность. Если он потребует новых полей в `Call`,
`RunRequest` или `Evidence` — это сигнал, что предыдущий пакет был неполным;
тогда возвращай `STATUS: SPLIT_REQUIRED` с перечнем недостающих, а не расширяй
`files_allowed` на соседние пакеты.

## Evidence

- `cargo test -p stellwerk-task --test vertical` → 0 failed.
- `cargo test -p stellwerk-task` → 0 failed целиком.
- Ручной прогон вертикального среза одним Task, зафиксированный PolyForge
  `ToolAttestation`, и `gate_evaluate passed`.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
