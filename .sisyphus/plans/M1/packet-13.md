# packet-13
plan_id: M1
goal: Ужесточить R3 (доступ к секретам) с `L4Admin` до `L5Owner` — правка контракта по решению владельца (ADR-0002).
files_allowed: [crates/stellwerk-core/src/contract.rs, crates/stellwerk-core/tests/risk_level_policy.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-core --test risk_level_policy
acceptance: >
  RED: сначала добавить утверждение `assert_eq!(RiskClass::R3.required_level(), PolicyLevel::L5Owner)`
  в tests/risk_level_policy.rs и зафиксировать вывод падения (Received L4Admin). Затем одной строкой
  заменить ветку R3 в contract.rs и добиться `risk_level_policy` → 0 failed. Итог по крейту: 55 passed.
  Тест `r4_r5_require_owner` и проверка монотонности `required_level_monotonic` обязаны продолжать
  проходить БЕЗ правок: их падение означает, что задето больше разрешённого.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки, изменение R0/R1/R2/R4/R5, переименование уровней, добавление классов риска]
depends_on: [06]
parallelism: sequential-only (single file lock — contract.rs)
data_class: public
lane: m1/core-risk
Contract-Change: RiskClass R3 required_level L4Admin -> L5Owner
Supersedes: M1-06 (R3 row only)
Reason: secret exposure is irreversible; L4 is granted per-workspace to employees and partners in the federation model
ADR: docs/adr/0002-r3-requires-owner.md

## Контекст

- Крейт `stellwerk-core`, единственный файл контракта — `src/contract.rs`.
- Контракт заморожен: правится ТОЛЬКО предикат, названный в этом пакете, и только
  одна его строка. Любая другая правка `contract.rs` — нарушение packet'а.
- `RiskClass::required_level` — единственный предикат контракта, который решает,
  сколько полномочий нужно для риска. Своих копий правил в gateway заводить нельзя.
- Ровно две строки таблицы затрагиваются этой правкой: R3 меняет уровень, R4
  **не меняет** его (уже `L5Owner`) — под R4 меняется только вердикт, а он живёт
  в M3 (`M3-02 permission.rs`), не в контракте.

## Что сделать

1. RED-шаг: в `crates/stellwerk-core/tests/risk_level_policy.rs` добавить
   ```rust
   /// R3 — доступ к секретам: необратим, поэтому только владелец (ADR-0002).
   #[test]
   fn r3_requires_owner() {
       assert_eq!(RiskClass::R3.required_level(), PolicyLevel::L5Owner);
   }
   ```
   Зафиксировать вывод падения: должно быть `assertion failed: ... (left: L4Admin, right: L5Owner)`.
2. GREEN-шаг: в `contract.rs`, в теле `required_level`, заменить **ровно одну**
   строку `RiskClass::R3 => PolicyLevel::L4Admin,` на `... => PolicyLevel::L5Owner,`.
   Строки R0, R1, R2, R4, R5 не трогать.
3. Проверить, что монотонность лестницы сохранена без правок: `L3Maintainer ≤
   L5Owner ≤ L5Owner ≤ L5Owner`. Если пришлось ослабить `required_level_monotonic`,
   задача выполнена неверно — вернуться и перечитать ADR-0002.
4. Проверить, что `r4_r5_require_owner` проходит без правок. Это сторожевой тест:
   он падает ровно тогда, когда задето лишнее.
5. Прогнать `cargo test -p stellwerk-core` целиком: падать не имеет права ничего,
   кроме затронутого утверждения до GREEN-шага.

## Evidence

- Вывод падающего теста (RED), затем вывод проходящего (GREEN) — оба обязательны.
- `cargo test -p stellwerk-core --test risk_level_policy` → passed; 0 failed.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
