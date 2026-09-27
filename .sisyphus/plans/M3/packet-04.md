# packet-04
plan_id: M3
goal: Реализовать `decide`: три независимых правила, любое Deny побеждает.
files_allowed: [crates/stellwerk-gateway/src/gate.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-gateway --test gate
acceptance: `cargo test -p stellwerk-gateway --test gate` → 12 passed; 0 failed (включая Suspended/approval_timeout и абсолютность R5).
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
   КАКОГО правила сработало (`risk_r5`, `unclassified_tool`, `insufficient_level`,
   `data_class_exceeds_model`).
2. Полномочия проверяй двумя предикатами: `caller.permits(Action::RunTask)` и
   `caller >= risk.required_level()`. Второе важнее: R4 при L4Admin — `Deny`,
   при L5Owner — `Ask` (ADR-0002, В1).
3. `data` проверяй только если `CallRequest` содержит binding. Поле `model` сделать
   `Option<&ModelBinding>`? НЕТ — сделай обязательным и заведи отдельный тест на
   отсутствие привязки: без привязки данные уйти не могут, значит Deny.
4. Никаких `unwrap` и паник: gate обязан вернуть вердикт, а не упасть.
5. `src/lib.rs`: `pub mod gate;` после существующих `pub mod`.

### В1 — `Deny` абсолютен только для R5

`Deny` перестаёт быть «любым запретом». Вердикт `Deny` неэскалируется уровнем
только для `R5`; `R4` при нехватке уровня даёт `Deny`, но при `L5Owner` — `Ask`.
Правило композиции сохраняется: любой `Deny` побеждает.

### В4 — `Ask` в неинтерактивном прогоне: Suspended, а не Deny

`Deny` **не должен быть самостоятельным исходом** — иначе «запрещено политикой» и
«никто не ответил» неразличимы в расследовании, а это две разные ситуации.
`Deny` — лишь дефолт по таймауту.

```text
Ask в неинтерактивном Run:
  → TaskStatus::Suspended
  → ApprovalRequest { run_id, tool, risk, args_hash, requested_at, expires_at }
  → запись в .stellwerk/inbox/<run_id>.json

Исходы:
  Approve          → Run возобновляется, разрешение на ОДНО действие
  Deny             → Run возобновляется, вызов отклонён, Run продолжается
  ApproveForTask   → кэш на task_id (для R2/R3; для R4 — только явно)
  timeout истёк    → Failed(reason = approval_timeout), НЕ Deny
```

- R5 **не suspend'ится никогда**: `Deny` немедленно, без записи в inbox.
- Таймаут по умолчанию 24ч; для R4 — 4ч.
- Разница между `Denied` и `approval_timeout` обязана быть видна и в статусе,
  и в evidence.

## Evidence

- `cargo test -p stellwerk-gateway --test gate` → passed.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
