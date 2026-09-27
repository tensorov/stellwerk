# packet-09
plan_id: M3
goal: RED: чек на каждый вызов — вызов без чека считается несостоявшимся.
files_allowed: [crates/stellwerk-gateway/src/receipt.rs, crates/stellwerk-gateway/tests/receipt.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-gateway --test receipt
acceptance: RED: пакет собирается, тест компилируется, прогон падает на заглушке.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [08]
data_class: public
lane: m3/receipt-red

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

1. `src/receipt.rs`:
   - `pub struct Receipt { pub call_id: String, pub tool: String, pub module: String,
     pub verdict: Verdict, pub risk: RiskClass, pub input_hash: String,
     pub output_hash: String, pub truncated: bool, pub duration_ms: u64, pub ts: String }`;
   - `pub fn to_event(&self, task_id: &str) -> Event` — **RED-заглушка**;
   - `ReceiptError` (thiserror): `NotImplemented`, `Hash(String)`.
2. Правила:
   - `input_hash`/`output_hash` — sha256 от соответствующих тел. Секреты в хеш не пишут,
     но и сами тела в чеке не кладутся: только хеши;
   - `to_event` даёт `Event` с `event_type = "gateway.call"`, `source = System`,
     `actor = module`, `workspace` заполняется, `risk` из чека;
   - `payload` обязан содержать `task_id` (иначе `list_by_task` не найдёт), `call_id`,
     `tool`, `verdict`, `truncated`;
   - чек пишется для КАЖДОГО вызова, включая запрещённые: запрет — тоже факт, который
     должен попасть в поток. Иначе «тихие» попытки не видны в расследовании.
3. Тесты в `tests/receipt.rs`: хеши совпадают с sha256 тел; `to_event` даёт
   `gateway.call` и заполненный payload; verdict Deny тоже даёт событие;
   пустой вывод хешится без паники.

## Evidence

- RED-вывод `cargo test -p stellwerk-gateway --test receipt`.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
