# packet-12
plan_id: M3
goal: Реализовать виртуальную MCP-сессию: маршрутизация, шейпинг, чеки. Тогда весь `stellwerk-gateway` GREEN.
files_allowed: [crates/stellwerk-gateway/src/session.rs, crates/stellwerk-gateway/src/lib.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-gateway
acceptance: `cargo test -p stellwerk-gateway` → все тесты крейта зелёные. `cargo test -p stellwerk-core` и `-p stellwerk-store` не меняются.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [11c]
data_class: public
lane: m3/session

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

1. Реализуй `handle` строго в указанном порядке. Особенно важно: **проверка
   смонтированности ДО decide** и **запись чека при Deny**.
2. **`Ask` в неинтерактивном Run — это `Suspended`, а НЕ `Deny`** (В4). Здесь
   раньше стояло «`Ask` без подтверждения = `Deny` с причиной `ask_unresolved`»,
   и это прямо противоречило RED-тесту packet-11: тот требовал `Suspended` и inbox,
   а этот пункт предписывал `Deny`. Тест падал бы на своей же реализации.

   ```
   Ask в неинтерактивном Run:
     → TaskStatus::Suspended
     → ApprovalRequest { run_id, tool, risk, args_hash, requested_at, expires_at }
     → запись в .stellwerk/inbox/<run_id>.json
   Исходы:
     Approve        → Run возобновляется, разрешение на ОДНО действие
     Deny           → Run возобновляется, вызов отклонён, Run продолжается
     ApproveForTask → кэш на task_id (R2/R3; для R4 — только явно)
     timeout истёк  → Failed(reason = approval_timeout), НЕ Deny
   ```

   `Deny` **не самостоятельный исход**, а дефолт по таймауту. «Запрещено
   политикой» и «никто не ответил» — разные ситуации, и они обязаны различаться
   и в статусе, и в чеке. Таймаут 24ч, для R4 — 4ч. **R5 не suspend'ится
   никогда**: `Deny` немедленно, без записи в inbox.
3. **`risk >= R2` → субагенты запрещены на уровне permission** (В3): при `risk` R2
   и выше вызов не создаёт субагентов, `subagent_depth` остаётся `0`. Проверяется
   гейтом, а не текстом в промпте: модель текстовые запреты игнорирует, что
   доказано наблюдением F1 — ловил нарушение только post-flight детектор. Тест на
   `subagent_depth == 0` задан в packet-11; здесь он должен перестать падать.
   Дыра, которую это закрывает, и её точная формулировка — в `docs/DEFERRED.md`.

4. Ошибка backend не должна терять чек: сначала сформируй `Receipt` с хешами того, что
   успело уйти, затем верни `SessionError::Tool`.
5. `src/lib.rs`: `pub mod session;`.
6. Заключительная проверка целостности: ни один путь в шейпинге/чеке не должен
   печатать секреты. Убедись, что в `Receipt` попадают только хеши — это проверяется
   тестами packet-10, здесь просто не сломай.
7. На этом M3 закрыт. Итог всего плана: `cargo test --workspace` зелёный, кроме
   заранее оставленных RED-зон, если они ещё есть — перечисли их в ответе.

## Evidence

- `cargo test -p stellwerk-gateway` → passed. Это закрывает M3.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
