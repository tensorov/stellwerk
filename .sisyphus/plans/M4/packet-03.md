# packet-03
plan_id: M4
goal: GREEN — `authorize` собирает `CallRequest` и спрашивает `decide`, без собственной копии правил.
files_allowed: [crates/stellwerk-task/src/runner.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-task --test permission_wire
acceptance: `cargo test -p stellwerk-task --test permission_wire` → 0 failed. `cargo test -p stellwerk-gateway` остаётся 27/27 — регрессии нет.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [02]
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
- `cargo test -p stellwerk-gateway` → 27/27, регрессии нет.
- clippy `-D warnings` и `fmt --check` чисто.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
