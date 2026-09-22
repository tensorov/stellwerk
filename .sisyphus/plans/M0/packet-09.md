# packet-09
plan_id: M0
goal: audit GREEN — реализация audit_manifest/audit_module на sync stdio JSON-RPC (как measure) + обязательное использование rmcp::model, wiring CLI audit, exit 0/1, реальный прогон на polyforge-mcp.
files_allowed: [crates/stellwerk-registry/src/cmd_audit.rs, crates/stellwerk-registry/src/cli.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-registry audit
acceptance: cargo test -p stellwerk-registry audit → 0; собранный `target/debug/stellwerk module audit` на полигоне (polyforge-mcp в PATH) → exit 0, stdout-отчёт без not_invocable; kind=skill в tempdir → exit 0 + deferred-сообщение; при выключенном polyforge-mcp → exit 1.
forbidden: [cargo add, правка корневого Cargo.toml, правка Cargo.toml крейта (rmcp уже добавлен в 08), правка modules.lock, правка contract.rs, правка lib.rs/model.rs, таймаут ≠10s в прод-пути (в тестах 50ms), #[ignore], #[allow], ослабление тестов packet-08]
depends_on: [packet-08]

## GREEN-пакет

Lane m0/audit: зелёные тесты 08 + реальная интеграция. Решение по транспорту ЗАФИКСИРОВАНО (не выбирай): **sync stdio JSON-RPC, тот же подход, что в cmd_measure** (initialize/tools/list/tools/call, `std::process`, без tokio-рантайма и без async rmcp-клиента). McpProbe trait из 08 остаётся sync.

`rmcp` в Cargo.toml (из 08) ОБЯЗАТЕЛЬНО live: в `cmd_audit.rs` сделай реальный импорт и использование хотя бы одного публичного item `rmcp::model` (тип/константа протокола — точный API смотри в исходниках крейта rmcp из lockfile), чтобы dep не был dead. `#[allow]` ЗАПРЕЩЁН; «можешь не трогать rmcp» — НЕТ, fallback на «чистый stdio без rmcp» отменён.

## Спецификация

1. `audit_manifest` (реализация из заглушки 08): сверка exact-set, вызовы call_tool("{}"), классификация CallOutcome, уважение timeout_ms (в impl — через chrono? нет; в sync-клиенте таймаут: spawn reader-thread с channel recv_timeout(timeout_ms) — std, без новых крейтов).
2. `audit_module(module, root, timeout_ms)`:
   - kind=skill → report { skill_skipped: true }, Ok.
   - kind=mcp → взять module.command, spawn std::process (NotFound → AuditError::Spawn, exit 1, без паники), initialize+tools/list+поштучные tools/call, собрать report. Прод-дефолт timeout_ms=10_000; тесты передают 50.
3. cli.rs dispatch Audit(NameArgs): name обязателен (как measure), cwd=root; match-arm в КОНЕЦ существующего match (чужие не трогай); печать отчёта человекочитаемо (missing/extra/not_invocable списки, «OK» если всё пусто); exit 0/1.
4. lib.rs уже содержит cmd_audit (08) — не трогай.

## Тесты (добавить, старые не ослаблять)

- Все fake-trait тесты 08 зелёные.
- Интеграционный: tempdir + sh-fixture «MCP server» (скрипт как в 07a, но tools/list отдаёт `{"tools":[{"name":"t1","inputSchema":{"type":"object"}},{"name":"t2","inputSchema":{"type":"object"}}]}`, а на `tools/call` с любым id — `{"jsonrpc":"2.0","id":<тот же id из запроса>,"result":{"content":[]}}`; id вытягивай из строки запроса sed'ом), modules.toml с command=путь_к_скрипту, tools=["t1","t2"] → audit OK exit 0.
- Fixture без t2 в манифесте → extra_in_server, exit 1.
- Fixture который не отвечает → Timeout → exit 1 (timeout 50ms в тесте).
- skill module → exit 0, stderr/stdout «deferred».
- Ручной acceptance: `target/debug/stellwerk module audit` с реальным polyforge-mcp (PATH) → 0; если нет бинаря → STATUS: BLOCKED в отчёте (не в тестах).

## Приёмка

- `cargo test -p stellwerk-registry audit` → 0.
- clippy -D warnings чисто (rmcp реально импортирован/использован в cmd_audit, без allow).
- Реальный прогон polyforge — в Evidence.

## Evidence

- GREEN вывод тестов; вывод реального audit polyforge; PROOF evidence_verify(cmd='cargo test -p stellwerk-registry audit'); GATE; STATUS/FILES/TEST/EVIDENCE.
