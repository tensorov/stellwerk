# packet-07
plan_id: M3
goal: RED: ленивый монтаж модулей — бюджет контекста выбирает, что вообще подключать.
files_allowed: [crates/stellwerk-gateway/src/mount.rs, crates/stellwerk-gateway/tests/mount.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-gateway --test mount
acceptance: RED: пакет собирается, тест компилируется, прогон падает на заглушке.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [06]
data_class: public
lane: m3/mount-red

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

1. `src/mount.rs`:
   - `pub struct Candidate { pub name: String, pub kind: ModuleKind, pub cost_tokens: u64,
     pub risk: Option<RiskClass>, pub risk_delta: i64 }`;
     `risk_delta` — насколько модуль меняет бюджет риска сессии (у PolyForge −1: он
     СНИМАЕТ риск, потому что даёт проверенный путь; у сетевого MCP +1);
   - `pub struct MountPlan { pub mounted: Vec<String>, pub skipped: Vec<(String, SkipReason)> }`;
   - `pub enum SkipReason { OverBudget, TooRisky, Duplicate, Disabled }`;
   - `pub fn plan(candidates: &[Candidate], budget_tokens: u64) -> Result<MountPlan, MountError>` — **RED-заглушка**.
2. Правила, которые задают тесты:
   - модули монтируются **по возрастанию `cost_tokens`** (дешёвые базовые инструменты
     первыми): порядок не должен зависеть от порядка входа — иначе две сессии с одним
     набором модулей получают разные бюджеты;
   - модуль не влезает в остаток бюджета → `SkipReason::OverBudget`, монтирование
     продолжается (жадный проход, а не отказ);
   - модуль, поднимающий риск выше `RiskClass::R2`, → `SkipReason::TooRisky`;
   - повторное имя → `SkipReason::Duplicate`;
   - `ModuleKind::Skill` не монтируется всегда: его цену уже платит контекст, а
     инструментов он не даёт — `SkipReason::Disabled` (проверь, что это решение
     осознанное, и опиши его в комментарии);
   - суммарная стоимость смонтированных ≤ бюджета: это главный инвариант.
3. Тесты в `tests/mount.rs` на детерминированных наборах.

## Evidence

- RED-вывод `cargo test -p stellwerk-gateway --test mount`.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
