# packet-09b
plan_id: M0
goal: audit GREEN, часть 2 — sync stdio JSON-RPC транспорт (write_message, read_response, разбор tools/list, поштучный tools/call), StdioProbe как живая реализация McpProbe, и ветка kind=mcp в audit_module. Транспорт обобщён над <R: BufRead, W: Write> и потому тестируется in-memory, без процесса. CLI и сквозные тесты вынесены в packet-09c.
files_allowed: [crates/stellwerk-registry/src/cmd_audit.rs, crates/stellwerk-registry/tests/audit_transport.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-registry
acceptance: `cargo test -p stellwerk-registry` → 0 failed; девять тестов packet-08 и девять тестов packet-09 остаются зелёными и побайтно неизменными; `cargo fmt --all -- --check` → чисто; `cargo clippy --all-targets -- -D warnings` → чисто.
forbidden: [cargo add, правка корневого Cargo.toml, правка Cargo.toml крейта (rmcp уже добавлен в 08), правка modules.lock, правка contract.rs, правка lib.rs/model.rs/cli.rs, правка modules.toml, реальный spawn MCP в тестах, async, tokio-рантайм, #[ignore], #[allow], ослабление тестов пакетов 08 и 09]
depends_on: [packet-08, packet-09]

## GREEN-пакет, часть 2 из 3

Пакет выделен из `M0-09b` после измерения. F-54 оценивала транспорт в 138 логических строк;
перепроверка дала **119**: `write_message` 6, `read_response` 40, `parse_tools_list_response` 28,
`run_session` 45 (все — в `crates/stellwerk-registry/src/cmd_measure.rs`, границы функций
измерены, логические строки — без пустых и без строк, первым идущих с `//`). К этому добавляется
`tools/call`, которого в крейте нет **ни одного** вхождения, то есть пишется с нуля.

Оба файла пакета раньше считались одним кошельком на 200 строк, потому что оба лежали в `src/`.
Теперь тесты вынесены в `crates/stellwerk-registry/tests/audit_transport.rs`, и ADR-0003 даёт
каталогу `tests/` **отдельный** бюджет 250 «по расположению», а не по имени файла. Это не
техническая уловка, а прямое следствие решения транспорта: обобщение над
`<R: BufRead, W: Write>` делает его тестируемым без процесса, ровно как уже сделано
`run_session` в `cmd_measure.rs` с явной пометкой «testable without a process».

Ожидаемая арифметика: `src/cmd_audit.rs` ≈ 119 транспорт + ~22 `tools/call` + ~20 `StdioProbe`
+ ~22 ветка `kind=mcp` = **≈183 логических строки при пределе 200**;
`tests/audit_transport.rs` ≈ 150 при пределе 250. Обе оценки — предварительные; фактическое
значение обязано быть измерено коммитом, а не сложено.

## Решение по транспорту ЗАФИКСИРОВАНО, не выбирай

**Sync stdio JSON-RPC, тем же подходом, что в `cmd_measure`**: `std::process`, `initialize` →
`notifications/initialized` → `tools/list` → поштучные `tools/call`. Без tokio и без
async-клиента `rmcp`. Трейт `McpProbe` из 08 остаётся **sync** и по сигнатуре не меняется.

Скопировать `cmd_measure.rs` нельзя: он вне `files_allowed`, а его `write_message` и
`read_response` приватные. Своя реализация поверх `std::io` — да.

**Тесты не поднимают процесс.** Транспорт обобщён над `R: BufRead, W: Write`, поэтому
`tests/audit_transport.rs` гоняет полный цикл `initialize` → `tools/list` → `tools/call` на
`&[u8]` в памяти: подставь входные строки, собери выход, проверь. Ни `Command`, ни `tempdir`, ни
зависимостей. Прецедент — `run_session`, уже обобщённый так же.

## `rmcp` ОБЯЗАТЕЛЬНО live

Зависимость добавлена в 08, но до этого пакета не использовалась — «мёртвая». Здесь нужен
**реальный импорт и реальное использование** хотя бы одного публичного item `rmcp::model` —
типа или константы протокола. Точный API смотри в исходниках крейта из lockfile
(`~/.cargo/registry/src/*/rmcp-3.4.0/src/model.rs`), **не угадывай имена**: в 3.4.0 есть
`rmcp::model::{CallToolResult, CallToolRequestParams, Tool, …}`, а имена менялись между
минорными версиями. `#[allow(dead_code)]`-подмена запрещена: это объявление о применении, а не
применение.

## Что сделать

1. **Транспорт** в `src/cmd_audit.rs`, обобщённый над `R: BufRead, W: Write`:
   - `write_message` — запись строки + `\n` + `flush`;
   - `read_response(reader, id)` — построчное чтение до ответа на нужный `id`, с пропуском
     уведомлений и границей на число прочитанных сообщений, чтобы сервер со сломанным
     кадрированием не закручивал вызывающего в бесконечный цикл (прецедент — `MAX_SKIPPED`
     в `cmd_measure.rs`);
   - разбор ответа `tools/list` в `Vec<String>` имён.
2. **`tools/call`** — запрос с подстановкой инкрементального `id`, ответ классифицируется в
   `CallOutcome`: `result` → `Result`, `error` → `Error` (тул жив, Q3), битый JSON или отсутствие
   и `result`, и `error` → `Malformed`. Таймаут различает `Timeout`.
3. **Таймаут честно и без новых крейтов**: reader-thread кладёт прочитанную строку в
   `std::sync::mpsc::channel`, вызывающий ждёт `recv_timeout(timeout_ms)`. Значение
   **внедряется**, не зашито: `AUDIT_TIMEOUT_MS = 10_000` в проде и 50 мс в тестах зафиксированы
   тестом из 08. `timeout_ms` из 08 перестаёт быть неиспользуемым параметром.
4. **`StdioProbe`** — реализация `McpProbe` поверх `std::process::Child` с piped stdin/stdout.
   `NotFound` при spawn → `AuditError::Spawn`. **Без `unwrap()` и без паники** на всём, что
   пришло от процесса: ошибка чтения/записи становится `Malformed`, а не аварий.
5. **`audit_module`, ветка `kind = mcp`:** взять `module.command`; `None` → `AuditError::NoCommand`.
   Далее initialize → `tools/list` → собрать `AuditReport` через уже готовый `audit_manifest`.
   Ветку `kind = skill` не переписывать — она закрыта в packet-09.
6. **`lib.rs` не трогать** — `pub mod cmd_audit` объявлен в 08.

## Тесты — `tests/audit_transport.rs`

Новый интеграционный файл, RED пишется первым. Кейсы:

- полный цикл на входе в памяти: `initialize` → `tools/list` отдаёт `t1`, `t2` → `tools/call`
  возвращает `result`; `CallOutcome::Result`, отчёт без расхождений;
- ответ с `error` на `tools/call` → `CallOutcome::Error`, тул остаётся вызываемым (Q3);
- битый JSON на `tools/call` → `CallOutcome::Malformed`;
- сервер не отвечает, тест ждёт с бюджетом 50 мс → `CallOutcome::Timeout`;
- уведомление `notifications/initialized` в потоке не должно сбивать чтение ответа на `id`;
- `tools/list` с лишним именем → `extra_in_server`, в `audit_manifest`, а не здесь.

## БЮДЖЕТ: измерь и верни SPLIT_REQUIRED, а не подгоняй

Лимит — 200 логических строк **добавленных коммитом** для `src/cmd_audit.rs` и 250 для
`tests/audit_transport.rs` (ADR-0003: без пустых и без строк, первым идущих `//`, у `tests/`
считаются полностью). Измерение обязательно и должно быть в отчёте: `git diff --numstat`, затем
исключение пустых строк и строк с `//`, **с разбивкой по файлам**.

**Если сумма по `src/cmd_audit.rs` больше 200 — верни `SPLIT_REQUIRED`** с точным числом,
разбивкой и предложением разреза. **Нельзя** сжимать тесты, выкашивать комментарии или упрощать
приёмку ради прохода. Пакет важнее лимита, но сокрытие превышения запрещено.

## Evidence

- GREEN вывод `cargo test -p stellwerk-registry`.
- Блок тестов пакетов 08 и 09 побайтно неизменен — это проверяется по diff, а не тестом.
- PROOF: `evidence_append(ModelClaim)` + `evidence_verify(cmd='cargo test -p stellwerk-registry')`
  + `gate_evaluate`.
- Отдельный коммит `chore(evidence): attest M0-09b after merge` с chain_tail.
