# packet-09b
plan_id: M0
goal: audit GREEN, часть 2 — живой транспорт sync stdio JSON-RPC (как в cmd_measure), audit_module для kind=mcp, обязательное использование rmcp::model, dispatch CLI audit с exit 0/1 и реальный прогон на polyforge-mcp.
files_allowed: [crates/stellwerk-registry/src/cmd_audit.rs, crates/stellwerk-registry/src/cli.rs]
max_diff_lines: 200
test: cargo test -p stellwark-registry audit
acceptance: `cargo test -p stellwark-registry audit` → 0 failed; собранный `target/debug/stellwerk module audit` на реальном polyforge-mcp (в PATH) → exit 0 и отчёт без not_invocable; `kind=skill` в tempdir → exit 0 с сообщением «deferred»; при недоступном сервере → exit 1, без паники; `cargo fmt --all -- --check` → чисто; `cargo clippy --all-targets -- -D warnings` → чисто.
forbidden: [cargo add, правка корневого Cargo.toml, правка Cargo.toml крейта (rmcp уже добавлен в 08), правка modules.lock, правка contract.rs, правка lib.rs/model.rs, правка modules.toml, таймаут ≠10s в прод-пути (в тестах 50ms), #[ignore], #[allow], ослабление тестов пакетов 08 и 09, async, tokio-рантайм]
depends_on: [packet-05, packet-07a, packet-09, packet-11]

## GREEN-пакет, часть 2 из 2

Пакет выделен из `M0-09` после измерения (F-54): транспорт, `cli.rs` и интеграционные тесты
не помещались вместе с `audit_manifest` в 200 логических строк. Часть 1 — `packet-09`, она
делает `audit_manifest` и skill-ветку `audit_module` зелёными. Здесь остаётся всё, что
живёт: spawn, stdio, JSON-RPC, `rmcp`, CLI, интеграционные тесты, ручной прогон.

`depends_on` объявлен на четыре пакета, и это порядок, а не формальность. `packet-09b`
пишет в `crates/stellwerk-registry/src/cli.rs`, и раньше его писали `packet-05`,
`packet-07a` и `packet-11`, а `packet-09` — в тот же `cmd_audit.rs`. Применить `09b`
до них нельзя: он правит код, которого ещё нет. Проверка C1 требует, чтобы пересечение
по файлам было упорядочено хотя бы одной стороной, и здесь это не синоним того, что
зависимость фиктивна.

## Решение по транспорту ЗАФИКСИРОВАНО, не выбирай

**Sync stdio JSON-RPC, тем же подходом, что в `cmd_measure`**: `std::process`, `initialize` →
`notifications/initialized` → `tools/list` → поштучные `tools/call`. Без tokio-рантайма и без
async-клиента `rmcp`. Трейт `McpProbe` из 08 остаётся **sync**.

Скопировать `cmd_measure.rs` нельзя: он вне `files_allowed`, а его `write_message` и
`read_response` приватные. Своя реализация поверх `std::io` — да.

## `rmcp` ОБЯЗАТЕЛЬНО live

Зависимость добавлена в 08, но до этого пакета не использовалась — «мёртвая». Здесь в
`cmd_audit.rs` нужен **реальный импорт и реальное использование** хотя бы одного публичного
item `rmcp::model` — тип или константа протокола. Точный API смотри в исходниках крейта из
lockfile (`~/.cargo/registry/src/*/rmcp-3.4.0/src/model.rs`), **не угадывай имена**: в
версии 3.4.0 есть `rmcp::model::{CallToolResult, CallToolRequestParams, Tool, …}`, и имена
менялись между минорными версиями. `#[allow(dead_code)]`-подмена запрещена: это не
использование, а объявление о нём.

## Что сделать

1. **Живой зонд** — реализация `McpProbe` поверх `std::process`: spawn с piped stdin/stdout,
   построчный обмен по JSON-RPC с подстановкой id, парсинг ответа в `rmcp::model`-типы,
   классификация не-успеха в `CallOutcome::{Timeout, Malformed}`. `NotFound` при spawn →
   `AuditError::Spawn`, **без `unwrap()` и без паники** на всём, что пришло от процесса.
2. **Таймаут честно.** Прод-путь — 10_000 мс, тесты — 50 мс. Реализация: reader-thread плюс
   `std::sync::mpsc::channel` и `recv_timeout(timeout_ms)`, без новых крейтов. Значение
   таймаута **внедряется**, а не зашито: константа `AUDIT_TIMEOUT_MS` и тестовые 50 мс
   зафиксированы тестом из 08 и не могут быть подменены.
3. **`audit_module`, ветка `kind = mcp`.** Взять `module.command`; `None` → `AuditError::NoCommand`.
   Далее initialize → `tools/list` → `tools/call` каждого заявленного тулла с `"{}"`, сбор
   `AuditReport`. Ветка `kind = skill` уже закрыта в packet-09 — не переписывать.
4. **CLI.** Dispatch-arm для уже разобранного `ModuleCommand::Audit(_)` (сейчас печатает
   «not implemented (packet-09)»). Arm — **в конец существующего `match`**, чужие arms не
   трогать. Имя модуля обязательно, как у `measure`; `cwd` — корень. Печать отчёта
   человекочитаемо: списки `missing` / `extra` / `not_invocable`, либо «OK», когда всё пусто.
   Код возврата 0 при пустых векторах расхождений, 1 иначе.
5. **`lib.rs` не трогать** — `pub mod cmd_audit` уже объявлен в 08.

## Тесты

- Интеграционный: tempdir + sh-фикстура «MCP server». Её `tools/list` отдаёт
  `{"tools":[{"name":"t1","inputSchema":{"type":"object"}},{"name":"t2",…}]}`, на `tools/call`
  с любым id — `{"jsonrpc":"2.0","id":<id из запроса>,"result":{"content":[]}}`; id вытягивать
  из строки запроса. `modules.toml` с `command` = путь к скрипту и `tools=["t1","t2"]` → отчёт
  пустой, exit 0.
- Фикстура, чей `tools/list` отдаёт лишний тул, не заявленный в манифесте → `extra_in_server`,
  exit 1.
- Фикстура, которая не отвечает → `Timeout` → `not_invocable`, exit 1 (50 мс в тесте).
- `kind = skill` → exit 0, «deferred». Уже закрыто тестом из 08, здесь не дублировать.
- Ручная приёмка (в отчёте, **не** в тестах): `target/debug/stellwerk module audit` с реальным
  `polyforge-mcp`. Если бинаря нет — `STATUS: BLOCKED` в отчёте, а не ослабление теста.

## БЮДЖЕТ: измерь и верни SPLIT_REQUIRED, а не подгоняй

Лимит — 200 логических строк **добавленных коммитом** (ADR-0003: без пустых и без строк,
первым идущих `//`). Оба файла в `src/`, поэтому интеграционные тесты в `cmd_audit.rs` идут в
**тот же** кошелёк: тестовый бюджет 250 даётся по расположению (`tests/` в пути), а такого
пути в `files_allowed` нет.

Измерение обязательно и должно быть в отчёте: `git diff --numstat`, затем исключить пустые
строки и строки с `//`, с разбивкой по файлам.

**Если больше 200 — верни `SPLIT_REQUIRED`** с точным числом, разбивкой по файлам и
предложением разреза, у которого у каждой части остаётся свой RED. **Нельзя** сжимать тесты,
выкашивать комментарии или упрощать приёмку ради прохода. Пакет важнее лимита, но сокрытие
превышения запрещено.

## Evidence

- GREEN вывод `cargo test -p stellwark-registry audit`.
- Реальный прогон `polyforge-mcp` — дословно, с кодом возврата.
- PROOF: `evidence_append(ModelClaim)` +
  `evidence_verify(cmd='cargo test -p stellwark-registry audit')` + `gate_evaluate`.
- Отдельный коммит `chore(evidence): attest M0-09b after merge` с chain_tail.
