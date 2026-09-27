# packet-11
plan_id: M3
goal: RED: виртуальная MCP-сессия — маршрутизация вызова по цепочке permission → gate → shaping → receipt.
files_allowed: [crates/stellwerk-gateway/src/session.rs, crates/stellwerk-gateway/tests/session.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-gateway --test session
acceptance: RED: пакет собирается, тест компилируется, прогон падает на заглушке.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [10]
data_class: public
lane: m3/session-red

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

1. `src/session.rs`:
   - `pub struct Session { pub id: String, pub plan: MountPlan, pub caller: PolicyLevel,
     pub budget_tokens: u64, pub calls: u32 }`;
   - `pub struct CallOutcome { pub verdict: Verdict, pub shaped: Option<Shaped>,
     pub receipt: Option<Receipt> }`;
   - `pub async fn handle(&mut self, req: &CallRequest, tool: &dyn ToolBackend)
     -> Result<CallOutcome, SessionError>` — **RED-заглушка**.
2. `ToolBackend` (трейт в этом же файле): `async fn invoke(&self, tool: &str, input: &str)
   -> Result<String, ToolError>`. Нужен, чтобы тесты не запускали настоящий MCP.
3. Порядок обработки (нарушение порядка ломает безопасность):
   1. инструмент есть в смонтированных? иначе `Verdict::Deny { reason: not_mounted }`
      и вызов НЕ доходит до backend;
   2. `decide(req)`; при `Deny` — чек пишется, `shaped` пуст, backend не вызывается;
   3. при `Allow`/`Ask` — вызов backend; при `Ask` подтверждение моделируется полем
      `ask_resolved: bool` в `CallRequest` (без подтверждения `Ask` трактуется как Deny);
   4. shaping результата;
   5. чек в поток.
4. Тесты в `tests/session.rs` с поддельным backend:
   - неизвестный инструмент → Deny, backend НЕ вызван;
   - Deny по риску → backend не вызван, чек есть;
   - Ask без подтверждения → backend не вызван;
   - Ask с подтверждением → вызван, shaped присутствует;
   - успешный вызов → ровно один чек, `calls` увеличился;
   - ошибка backend → `SessionError::Tool`, чек всё равно записан (неудача видна).

## Evidence

- RED-вывод `cargo test -p stellwerk-gateway --test session`.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
