# packet-04
plan_id: M3
goal: Реализовать `decide`: три независимых правила, любое Deny побеждает.
files_allowed: [crates/stellwerk-gateway/src/gate.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-gateway --test gate
acceptance: `cargo test -p stellwerk-gateway --test gate` → 0 failed, и каждый из 17 сценариев, перечисленных в M3/packet-03 (п.3), утверждён хотя бы одним `#[test]`. Счётчик тестов фиксировать нельзя: группировка сценариев в одну функцию допустима, и жёсткое «15 passed» ломало бы пакет при любой разнице группировки. Покрытие проверяется по сценариям, а не по числу функций.
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

### Ось полномочий живёт здесь, а не в `permission_for`

Два утверждения из В1/В2 невозможно проверить в `permission_for`, потому что
она не получает ни уровня, ни признака интерактивности. Оба — здесь, где есть и
то, и другое:

- `R4` при `L4Admin` и ниже → `Deny`; при `L5Owner` → `Ask` (В1). Уровень берётся
  из контракта, `caller` — из `CallRequest`.
- `Unclassified` в **интерактивной** сессии `L5Owner` → `Ask`; в неинтерактивной →
  `Deny`, без suspend (В2). `permission_for` для неинтерактивной части уже
  отвечает `Deny`; здесь проверяется, что интерактивная ветка действительно
  существует и что она не превращается в молчаливый `Allow`.

Тесты на эти два случая пишутся здесь, а не в M3-01: `packet-01` не имеет
права создавать тесты, а `packet-02` не имеет права их править. Если эти
утверждения не покрыть здесь, они не будут покрыты нигде.

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
