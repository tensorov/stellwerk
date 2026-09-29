# packet-08
plan_id: M0
goal: audit RED — добавить rmcp (единственная правка Cargo.toml в M0), sync-транспорт-независимый trait-шов McpProbe + заглушки, падающие тесты отчёта/валидации.
files_allowed: [crates/stellwerk-registry/Cargo.toml, crates/stellwerk-registry/src/lib.rs, crates/stellwerk-registry/src/cmd_audit.rs, Cargo.lock]
limit_waiver: 4 файла при лимите 3, и разбить нельзя: rmcp не был в Cargo.lock, поэтому единственная правка зависимости тянет за собой пересборку лока силами cargo (45 пакетов в прошлом attempt f461b39). Лок не пишет автор пакета. Прецедент: M4-01 вносит Cargo.lock в files_allowed, M2-01 и M3-01 объявляют waiver на ту же причину.
max_diff_lines: 200
test: cargo test -p stellwerk-registry audit
acceptance: RED: bash -c 'cargo build -p stellwerk-registry && cargo test -p stellwerk-registry audit --no-run && ! cargo test -p stellwerk-registry audit' → exit 0; в Cargo.toml крейта появилась строка `rmcp = { workspace = true }` (workspace уже объявлен в корне); корневой Cargo.toml не изменён.
forbidden: [cargo add, правка корневого Cargo.toml, правка modules.lock, правка contract.rs, правка cli.rs/main.rs/model.rs, реальный spawn MCP, #[ignore], #[allow], исправление теста]
depends_on: [packet-04]

## RED-пакет — правила

DO NOT fix/red-green the test inside a RED packet. Зелёным сделает packet-09 (lane-audit). Реальный транспорт (stdio/rmcp) — в 09; здесь только зависимости + sync-шов + фейковые тесты. Trait остаётся sync (`&mut self`) — async в M0 не тянем.

## Контекст

- audit: только kind=mcp; kind=skill → пропуск с deferred-сообщением (exit 0, «skill audit deferred»).
- Флоу: spawn command → initialize → tools/list → сверка exact-set имён с manifest `module.tools` → для каждого тулла tools/call с `{}` (invalid-args probe, Q3) → любой валидный JSON-RPC result|error = invocable; таймаут/битый ответ = fail.
- Timeout: 10s на тул (в тестах инжектируемый, 50ms).
- Exit: 0 — все заявленные ок + списки совпали; 1 — расхождение/timeout/недоступен.
- rmcp = workspace dep (уже в корне Cargo.toml с features client, transport-child-process). Добавить в [dependencies] крейта: `rmcp = { workspace = true }`. Это ЕДИНСТВЕННАЯ правка Cargo.toml во всём M0 — так и оставь в комментарии.

## Что сделать

1. crates/stellwerk-registry/Cargo.toml: добавить `rmcp = { workspace = true }` в [dependencies].
2. lib.rs: добавить `pub mod cmd_audit;` в АЛФАВИТНУЮ позицию между `cmd_add` и `cmd_measure`. Раньше здесь было «append ПОСЛЕ последней `pub mod`», и это несовместимо с `cargo fmt --check`: rustfmt с `reorder_modules` (дефолт true) переставляет `pub mod` по алфавиту, и пакет в исходном виде заставляет fmt требовать перестановки, то есть исполнитель вынужден выбрать между буквой пакета и зелёным fmt. Алфавитная позиция удовлетворяет обоим.
3. cmd_audit.rs — шов + заглушки (RED):
   - `pub trait McpProbe { fn list_tools(&mut self) -> Result<Vec<String>, AuditError>; fn call_tool(&mut self, name: &str, args: &str) -> Result<CallOutcome, AuditError>; }`
   - `CallOutcome`: `Result(serde_json::Value)` | `Error(serde_json::Value)` | `Timeout` | `Malformed`.
   - `pub struct AuditReport { pub missing_in_server: Vec<String>, pub extra_in_server: Vec<String>, pub not_invocable: Vec<String>, pub skill_skipped: bool, ... }`
   - `pub fn audit_manifest(tools: &[String], probe: &mut dyn McpProbe, timeout_ms: u64) -> Result<AuditReport, AuditError>` — RED: `Err(NotImplemented)` или пустой отчёт ломающий assert (выбери Err чтобы тесты падали явно).
   - `pub fn audit_module(module: &Module, root: &Path, timeout_ms: u64) -> Result<AuditReport, AuditError>` — заглушка; реальный rmcp — packet-09. Для skill: можно сразу в 08 сделать ветку skill→skip (это легко и не требует rmcp) — допустимо в GREEN-мысли, но в RED оставь NotImplemented чтобы тест падал; точную раскладку определи тест.
4. Время: без tokio в тестах — audit_manifest чистая функция над trait, таймауты моделируются фейком возвращающим Timeout.

## Тесты (filter `audit`, в cmd_audit.rs tests)

1. FakeProbe: manifest [a,b,c], server [a,b,c], все call → Result → report пустые fail-векторы, skill_skipped=false.
2. Server [a,b] (нет c) → missing_in_server=[c].
3. Server [a,b,c,d] → extra_in_server=[d].
4. call c → Timeout → not_invocable=[c]; call b → Malformed → not_invocable=[b].
5. call → JSON-RPC error object (валидный) → invocable (не в not_invocable) — Q3.
6. kind=skill → audit_module/report: skill_skipped=true, exit-семантика OK (нет проброса spawn).
7. timeout_ms параметр пробрасывается (проверить что фейк получил 50 vs 10000 — через ячейку в FakeProbe).
8. audit_manifest пустой manifest + пустой сервер → OK (ничего не заявлено — vacuous pass).

RED: эти assert'ы падают на Err(NotImplemented)/пустых.

## Evidence

- RED wrapper → 0 + вывод.
- STATUS/FILES/TEST/EVIDENCE.
