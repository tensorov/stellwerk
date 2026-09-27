# packet-04
plan_id: M3
goal: Реализовать `decide`: три независимых правила, любое Deny побеждает.
files_allowed: [crates/stellwerk-gateway/src/gate.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-gateway --test gate
acceptance: `cargo test -p stellwerk-gateway --test gate` → 8 passed; 0 failed.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [03]
data_class: public
lane: m3/gate

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

1. Порядок: риск → полномочия → данные. Deny возвращается немедленно, с указанием
   КАКОГО правила сработало (`risk_r4`, `insufficient_level`, `data_class_exceeds_model`).
2. Полномочия проверяй двумя предикатами: `caller.permits(Action::RunTask)` и
   `caller >= risk.required_level()`. Второе важнее: R4 при L4Admin всё равно Deny.
3. `data` проверяй только если `CallRequest` содержит binding. Поле `model` сделать
   `Option<&ModelBinding>`? НЕТ — сделай обязательным и заведи отдельный тест на
   отсутствие привязки: без привязки данные уйти не могут, значит Deny.
4. Никаких `unwrap` и паник: gate обязан вернуть вердикт, а не упасть.
5. `src/lib.rs`: `pub mod gate;` после существующих `pub mod`.

## Evidence

- `cargo test -p stellwerk-gateway --test gate` → passed.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
