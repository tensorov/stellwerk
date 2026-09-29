# packet-03
plan_id: M4
goal: GREEN — `authorize` собирает `CallRequest` и спрашивает `decide`, без собственной копии правил.
files_allowed: [crates/stellwerk-task/src/runner.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-task --test permission_wire
acceptance: `cargo test -p stellwerk-task --test permission_wire` → 0 failed из 10. `cargo test -p stellwerk-gateway` остаётся 62/62 зелёных (gate 11, gate_ask 7, mount 8, permission 9, receipt 12, session 4, session_contract 5, shaping 6) — регрессии нет. `session_failure` остаётся красным 0/4: это фикстура M3-11c, её озеленит M3-12b, и её озеленить здесь нельзя — это был бы обход F-31.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [02, M2-09, M2-10, M2-11, M2-13, M2-14]
data_class: public
lane: m4/authorize

## Контекст

- Единственная цель пакета — сделать `decide` **достижимым** из `stellwerk-task`.
  Это закрывает F-10 на уровне кода: после пакета существует путь
  `раннер → authorize → decide`, и он единственный.
- `ToolRisk`, `Permission`, `Verdict`, `decide` — из `stellwerk_gateway::gate`
  и `stellwerk_gateway::permission`. `RiskClass`, `PolicyLevel`, `DataClass`,
  `ModelBinding` — из `stellwerk_core::contract`.

## Что сделать

1. `authorize(call, caller) -> Verdict` собирает
   `CallRequest { tool, module, risk: call.risk, caller, data: call.data, model: call.model, interactive: call.interactive }`
   и возвращает `decide(&request)`. **Одна строка делегирования, без разбора
   вердиктов и без собственной таблицы.**
2. Никакой интерпретации `Verdict` здесь не делать: что делать с `Allow`, `Ask`,
   `Suspended` и `Deny` — это packet-05/07. Этот пакет отвечает на один вопрос
   «кто решает», а не «что делать дальше».
3. Импорт `stellwerk_gateway` теперь возможен — зависимость добавлена packet-01.

## Почему запрещено разбирать `Verdict` в этом пакете

`Verdict` разбирается в трёх местах (диспетчеризация, чек, approval), и разбор в
четвёртом создал бы вторую копию правил — ровно тот класс, который M3 устраняет
повсюду. Если кажется, что разбирать нужно больше, чем в трёх местах, это признак
того, что одного из них не хватает, а не что нужен четвёртый.

## Evidence

- `cargo test -p stellwerk-task --test permission_wire` → 0 failed.
- `cargo test -p stellwerk-gateway` → 62/62 зелёных, регрессии нет. Число 27, стоявшее здесь
  раньше, не соответствовало коду и было исправлено при раздаче пакета (класс F-29).
- clippy `-D warnings` и `fmt --check` чисто.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
