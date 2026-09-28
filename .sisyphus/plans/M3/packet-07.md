# packet-07
plan_id: M3
goal: RED: ленивый монтаж модулей — бюджет контекста выбирает, что вообще подключать.
files_allowed: [crates/stellwerk-gateway/src/mount.rs, crates/stellwerk-gateway/tests/mount.rs, crates/stellwerk-gateway/src/lib.rs]
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
     pub risk: ToolRisk, pub risk_delta: i64 }`;
     `risk_delta` — насколько модуль меняет бюджет риска сессии (у PolyForge −1: он
     СНИМАЕТ риск, потому что даёт проверенный путь; у сетевого MCP +1);
   - `pub enum ToolRisk { Classified(RiskClass), Unclassified }` — **В2, не
     `Option<RiskClass>`**. `Option` не выражает «риск неизвестен» как отдельное
     состояние: `None` молча превращается в `R0`, то есть отсутствие данных
     становится разрешением. `Unclassified` — состояние, а не пустота;
   - `pub struct MountPlan { pub mounted: Vec<String>, pub skipped: Vec<(String, SkipReason)> }`;
   - `pub enum SkipReason { OverBudget, TooRisky, Duplicate, Disabled, Unclassified }`;
   - `pub fn plan(candidates: &[Candidate], budget_tokens: u64) -> Result<MountPlan, MountError>` — **RED-заглушка**.
2. Правила, которые задают тесты:
   - модули монтируются **по возрастанию `cost_tokens`** (дешёвые базовые инструменты
     первыми): порядок не должен зависеть от порядка входа — иначе две сессии с одним
     набором модулей получают разные бюджеты;
   - модуль не влезает в остаток бюджета → `SkipReason::OverBudget`, монтирование
     продолжается (жадный проход, а не отказ);
   - модуль с `ToolRisk::Unclassified` → `SkipReason::Unclassified`, **никогда** `R0`
     и никогда `Allow` (В2). Причина пропуска обязана быть различима: «не
     классифицирован» и «дорого» — разные вещи, и в расследовании их нельзя
     смешивать;
   - отсечение по риску касается **только R5** → `SkipReason::TooRisky`.
     Р3 и R4 монтируются: их вердикт `Ask` выносится на момент вызова в `decide`
     (M3-04), и отсекать их здесь означало бы сделать решения В1 (R4 одобряем
     владельцем на одно действие) и В4 (`Ask` → `Suspended`) недостижимым кодом.
     Монтаж решает, что влезает в бюджет контекста, а не что разрешено;
   - повторное имя → `SkipReason::Duplicate`;
   - `ModuleKind::Skill` не монтируется всегда: его цену уже платит контекст, а
     инструментов он не даёт — `SkipReason::Disabled` (проверь, что это решение
     осознанное, и опиши его в комментарии);
   - суммарная стоимость смонтированных ≤ бюджета: это главный инвариант.
3. Тесты в `tests/mount.rs` на детерминированных наборах.

1b. `src/lib.rs`: добавить `pub mod mount;` одной строкой, ПОСЛЕ существующих
   `pub mod gate;`, `pub mod permission;`, `pub mod shaping;`. **Это обязательно
   здесь, а не в GREEN-пакете.** Интеграционный тест обращается к типам из `mount`
   через публичный API крейта, и без объявления модуля он не компилируется. Ровно
   эта ошибка уже стоила двух пакетов: в M2 требование `pub mod lifecycle;` висело
   на GREEN, а в M3 `lib.rs` не входил в `files_allowed` ни одного пакета, и
   `shaping` остался недостижим при зелёных тестах.

## Evidence

- RED-вывод `cargo test -p stellwerk-gateway --test mount`.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
