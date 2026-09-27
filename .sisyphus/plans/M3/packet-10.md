# packet-10
plan_id: M3
goal: Реализовать чеки вызовов и их запись в поток событий.
files_allowed: [crates/stellwerk-gateway/src/receipt.rs, crates/stellwerk-gateway/src/lib.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-gateway --test receipt
acceptance: `cargo test -p stellwerk-gateway --test receipt` → 6 passed; 0 failed.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [09]
data_class: public
lane: m3/receipt

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

1. Хеши: `sha2::Sha256` по байтам, hex в нижнем регистре. Хеш считай по полному выводу
   ДО shaping (иначе после усечения хеш не проверить).
2. `to_event` — чистая функция, без ввода-вывода: её удобно покрыть тестами.
3. `duration_ms` — `u64` миллисекунд; не используй `f32`/`f64` для времени.
4. `src/lib.rs`: `pub mod receipt;`.
5. Запись в хранилище — через `EventStore::append`. Сам вызов `append` НЕ делай здесь:
   вызывающий код (packet-11/12) решает, в какой момент писать, чтобы не записать
   событие для вызова, который не состоялся.

## Evidence

- `cargo test -p stellwerk-gateway --test receipt` → passed.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
