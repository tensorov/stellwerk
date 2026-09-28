# packet-03
plan_id: M3
goal: RED: итоговое решение по вызову — риск инструмента, полномочия пользователя и класс данных сводятся в один вердикт.
files_allowed: [crates/stellwerk-gateway/src/gate.rs, crates/stellwerk-gateway/tests/gate.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-gateway --test gate
acceptance: RED: пакет собирается, тест компилируется, прогон падает на заглушке.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [02]
data_class: public
lane: m3/gate-red

## Контекст

- **Новый крейт `stellwerk-gateway`** — виртуальный MCP-сервер: он НЕ выполняет
  инструменты сам, а решает, что можно, спрашивает, когда надо, и пишет чек.
- Крейт зависит от `stellwerk-core` и `stellwerk-store` по path. Новых крейтов нет;
  `cargo add` запрещён.
- Решения принимаются ТОЛЬКО через предикаты контракта:
  `RiskClass::required_level`, `PolicyLevel::permits`, `DataClass::permits_model`
  (все реализованы в M1, packet-05/06/04). Своих копий правил не заводить.
- Схемы MCP-модулей уже измерены: `measure` даёт `context_cost_tokens` (polyforge = 813).
  Gateway решает, что монтировать, по этим числам, а не по вкусу.
- Ключи и секреты не читать и не логировать: `auth.json`, `models.json`, `env | grep KEY`
  запрещены (`AGENTS.md`).

## Что сделать

1. `src/gate.rs`:
   - `pub struct CallRequest { tool: String, module: String, risk: RiskClass,
     caller: PolicyLevel, data: DataClass, model: ModelBinding }`;
   - `pub enum Verdict { Allow, Ask, Deny { reason: String } }`;
   - `pub fn decide(req: &CallRequest) -> Verdict` — **RED-заглушка**.
2. Правила решения (три независимых, порядок важен):
   1. **Риск инструмента** (`permission_for`): Deny → `Deny` сразу, Ask → кандидат на Ask;
   2. **Полномочия** (`PolicyLevel::permits` на действие `Action::RunTask` + сравнение с
      `RiskClass::required_level`): у вызывающего нет нужного уровня → `Deny`;
   3. **Данные** (`DataClass::permits_model`): класс данных выше, чем binding держит →
      `Deny` (это и есть «Personal не уходит в Big Pickle»).
   Итог: Deny по любому пункту; иначе Ask, если риск R2/R3/R4; иначе Allow.
   R5 — Deny всегда, он не попадает в ветку Ask (ADR-0002, В1).
3. Тесты в `tests/gate.rs`:
   - R0 + L2 + Public + first-party binding → Allow;
   - R1 + L0 → Deny (нет прав писать);
   - R2 + L2 → **Deny**: R2 требует `L3Maintainer`, у L2 нет прав (правило 2
     срабатывает раньше Ask — этот случай раньше был записан как «R2 + L2 → Ask»,
     что противоречило собственному правилу пакета);
   - R2 + L3 → Ask (спрашиваем);
   - R3 + L4 → **Deny**: после ADR-0002 R3 требует `L5Owner`, L4 недостаточен;
   - R3 + L5 → Ask (человек решает доступ к секретам);
   - R4 + L4 → Deny; R4 + L5 → **Ask на одно действие** (В1), не Deny;
   - R5 → Deny при любом уровне, включая L5Owner (необратимо — не делегируется);
   - Personal + third-party binding → Deny, даже при R0 и L5Owner;
   - Public + third-party → не Deny по данным.
4. `Verdict::Deny` обязан нести причину: без неё агент не поймёт, что делать.

## Evidence

- RED-вывод `cargo test -p stellwerk-gateway --test gate`.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
