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
   3. при `Allow` — вызов backend. При `Ask` — НЕ Deny: в неинтерактивном Run
      это `TaskStatus::Suspended` + `ApprovalRequest { run_id, tool, risk, args_hash,
      requested_at, expires_at }` + запись в `.stellwerk/inbox/<run_id>.json`,
      backend НЕ вызывается.
   4. shaping результата;
   5. чек в поток.
4. Тесты в `tests/session.rs` с поддельным backend:
   - неизвестный инструмент → Deny, backend НЕ вызван;
   - Deny по риску → backend не вызван, чек есть;
   - Ask в неинтерактивном Run → backend не вызван, статус `Suspended`, в inbox
     лежит `ApprovalRequest` с `args_hash`; это НЕ `Deny`;
   - истёкший `Ask` → `Failed(reason = approval_timeout)`, и это НЕ `Deny` —
     тест обязан различать `Denied` и `approval_timeout`;
   - R5 → `Deny` немедленно, `Suspended` не возникает, inbox пуст;
   - `risk >= R2` → субагенты запрещены: `subagent_depth == 0` (см. В3 ниже);
   - успешный вызов → ровно один чек, `calls` увеличился;
   - ошибка backend → `SessionError::Tool`, чек всё равно записан (неудача видна).

### В3 — субагенты запрещены при `risk >= R2`, на уровне permission

`decide` вызывается **до** запуска процесса, поэтому вызовы, порождённые внутри
процесса (субагент), повторно через gateway не проходят. Это дыра, и её не
закрывает ни один пакет M3.

- Запрет обязателен на **уровне permission**, а не текстом в промпте: модель
  текстовые запреты игнорирует (наблюдение F1: ловил только post-flight детектор).
  Значит `subagent_depth = 0` (или эквивалент) для любого Run с `risk >= R2`,
  **проверяемый гейтом**, а не инструкцией.
- В `DEFERRED` записывается не «изоляция», а точная формулировка дыры:
  *«decide вызывается до запуска процесса; вызовы, порождённые внутри процесса,
  повторно через gateway не проходят»*. Через полгода это будет единственное,
  что позволит понять, что именно чинить.
- Полноценная изоляция (podman/bubblewrap, namespace) остаётся в `DEFERRED`
  вместе с namespace-песочницей.

## Evidence

- RED-вывод `cargo test -p stellwerk-gateway --test session`.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
