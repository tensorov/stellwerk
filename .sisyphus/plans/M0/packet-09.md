# packet-09
plan_id: M0
goal: audit GREEN, часть 1 — audit_manifest над трейтом McpProbe плюс skill-ветка audit_module. Зеленит девять тестов, оставленных красными пакетом 08. Транспорт и CLI вынесены в packet-09b: в одном пакете они не укладывались в 200 строк.
files_allowed: [crates/stellwerk-registry/src/cmd_audit.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-registry audit
acceptance: `cargo test -p stellwerk-registry audit` → 0 failed, все девять тестов `cmd_audit::tests` зелёные; `cargo fmt --all -- --check` → чисто; `cargo clippy --all-targets -- -D warnings` → чисто.
forbidden: [cargo add, правка корневого Cargo.toml, правка Cargo.toml крейта, правка modules.lock, правка contract.rs, правка lib.rs/model.rs/cli.rs, реальный spawn MCP, async, #[ignore], #[allow], ослабление или удаление тестов пакета 08]
depends_on: [packet-08]

## GREEN-пакет, часть 1 из 2

Пакет сужен после измерения (F-54): в исходном виде он объявлял `cmd_audit.rs` и `cli.rs`
вместе с транспортом, и эквивалент транспорта в `cmd_measure.rs` стоит 138 логических строк
при пределе 200, а оба файла лежат в `src/`, где тестовый бюджет 250 не действует
(ADR-0003 раздаёт его по расположению, `tests/` в пути, которого в `files_allowed` нет).
Переиспользовать `cmd_measure.rs` нельзя: он вне `files_allowed`, его `write_message` и
`read_response` приватные, а публичная `run_session` возвращает measure-специфичный
`SessionOut` и не делает `tools/call`.

Трейт `McpProbe` из 08 остаётся **sync** (`&mut self`) и здесь не меняется. Никакого spawn.

## Что сделать

1. `audit_manifest` — реализация вместо заглушки `Err(NotImplemented)`:
   - сверка **exact-set** имён: чего нет на сервере → `missing_in_server`; что есть на сервере,
     но не заявлено → `extra_in_server`;
   - для каждого **заявленного** тулла `probe.call_tool(name, "{}")` — проба невалидными
     аргументами, потому что валидный JSON-RPC ответ сам по себе доказывает вызываемость
     (Q3 из packet-08);
   - классификация: `CallOutcome::Result` и `CallOutcome::Error` → тул вызываем, в
     `not_invocable` **не** попадает; `CallOutcome::Timeout` и `CallOutcome::Malformed` →
     `not_invocable`;
   - пустой манифест и пустой сервер → все векторы пусты, `skill_skipped == false`
     (vacuous pass: ничего не заявлено — нечего сверять).
2. `audit_module` — только skill-ветка: `kind = skill` → `Ok(AuditReport { skill_skipped: true,
   ..пусто })`. Это разрешено и не требует `rmcp`: packet-08 прямым текстом относит
   `kind=skill` к пропуску с отложенным сообщением, а RED-оставлял эту ветку незакрытой
   именно потому, что пакет объявлен красным. Ветка `kind=mcp` остаётся
   `Err(NotImplemented)` — это работа packet-09b.
3. `timeout_ms` здесь **не** используется: таймаут требует живого транспорта, а фейк
   моделирует таймаут исходом `CallOutcome::Timeout`. Прод-константа `AUDIT_TIMEOUT_MS = 10_000`
   и тестовая `50` остаются как объявлены в 08 и зафиксированы тестом.

## Чего этот пакет НЕ делает

Ни spawn, ни stdio, ни JSON-RPC, ни `rmcp`, ни CLI, ни интеграционных тестов. Живое
использование `rmcp::model`, требуемое исходным packet-09, выполняется в packet-09b вместе с
реальным транспортом — до него зависимость остаётся неиспользованной, что временно и гейтом
не проверяется.

## Приёмка

- `cargo test -p stellwark-registry audit` → 0 failed.
- `fmt` и `clippy -D warnings` чисты.
- Девять тестов пакета 08 остаются **в исходном виде**: ни ослаблены, ни удалены, ни
  пропущены. Это проверяется глазами по диффу, а не тестом.

## Evidence

- GREEN вывод `cargo test -p stellwark-registry audit`.
- PROOF: `evidence_append(ModelClaim)` + `evidence_verify(cmd='cargo test -p stellwark-registry
  audit')` + `gate_evaluate`.
- Отдельный коммит `chore(evidence): attest M0-09 after merge` с chain_tail.
