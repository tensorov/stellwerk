# packet-08
plan_id: M3
goal: Реализовать планировщик ленивого монтажа: жадный проход по цене, отсев по риску.
files_allowed: [crates/stellwerk-gateway/src/mount.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-gateway --test mount
acceptance: `cargo test -p stellwerk-gateway --test mount` → 8 passed; 0 failed. Сумма стоимости смонтированных всегда ≤ бюджета.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [07]
data_class: public
lane: m3/mount

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

1. Сортируй кандидатов по `(cost_tokens, name)` — второе поле делает порядок
   детерминированным при равных ценах.
2. Жадный проход: не влезает → `SkipReason::OverBudget`, продолжай. Отказ от всего
   набора из-за одного дорогого модуля хуже: дешёвые инструменты нужны больше.
3. Риск: `risk.unwrap_or(R0) + risk_delta`, с отсечением по R5. Если результат выше R2 →
   `SkipReason::TooRisky`.
4. Инвариант бюджета держи насыщением: `spent + cost <= budget`. Проверь целочисленным
   сложением, без знакового переполнения (u64 и `i64` смешивать нельзя).
5. Никаких `unwrap` на данных извне: неизвестный `risk` → R0.

## Evidence

- `cargo test -p stellwerk-gateway --test mount` → passed.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
