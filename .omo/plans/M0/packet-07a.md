# packet-07a
plan_id: M0
goal: measure GREEN — sync JSON-RPC stdio клиент (initialize + tools/list), расчёт байтов/токенов, запись tempdir modules.lock, wiring CLI measure.
files_allowed: [crates/stellwerk-registry/src/cmd_measure.rs, crates/stellwerk-registry/src/cli.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-registry measure
acceptance: cargo test -p stellwerk-registry measure → exit 0 (все тесты packet-06 + новые про spawn/клиент); ручной: собрать бинарь, в tempdir с modules.toml+fixture-сервером `stellwerk module measure <name>` печатает `measure <name>: schema_bytes=<B> context_cost_tokens=<T>` и пишет lock. Реальный polyforge-mcp — packet-07b, не здесь.
forbidden: [cargo add, правка корневого Cargo.toml, реального modules.lock/modules.toml репо, правка contract.rs, правка lib.rs, правка model.rs, токенизатор, сеть (только stdio локального процесса), #[ignore], #[allow], ослабление тестов packet-06]
depends_on: [packet-06]

## GREEN-пакет

Lane m0/measure: сделай зелёными тесты 06 и добавь клиент spawn. Верbatim для Momus (оставить в комментарии):

> 813 ±10% — это context_input-дельта измерения OpenCode (docs/CONTEXT_BUDGET.md), включая оверхед форматирования тулов, НЕ чистые байты схем; тест — pinning round-trip (bytes × tokens_per_byte ≈ 813), не проверка правильности коэффициента.

## Спецификация клиента (sync, std::process, без tokio)

```rust
pub fn measure_module(root: &Path, name: &str, command: &str) -> Result<MeasureReport, MeasureError>
```

1. Загрузи `ModulesFile` из `root/modules.toml`, возьми модуль `name` (нет → Err). command бери из args CLI / из модуля (module.command).
2. `std::process::Command::new(command)` (плюс аргументы не нужны — stdio JSON-RPC), stdin=pipe, stdout=pipe, stderr=inherit/piped.
3. Протокол (JSON-RPC 2.0, по одной строке JSON на сообщение, \n-terminated):
   - Отправь `initialize`: `{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"stellwerk","version":"0.0.1"}}}`
   - Прочитай ответ до \n; пришли notification `{"jsonrpc":"2.0","method":"notifications/initialized"}`.
   - Пришли `{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}`, прочитай ответ. protocolVersion: клиент шлёт "2024-11-05"; ответ инициализации без `result.protocolVersion` (несовместимый фрейминг) → Err(Protocol), без паники.
4. `parse_tools_list_response` → `schema_bytes` → `measure_tokens(B, calibration.tokens_per_byte)`.
5. `upsert_lock` в `root/modules.lock` (создай если нет: `version = 1\n`).
6. Верни отчёт; cli печатает: `measure {name}: schema_bytes={B} context_cost_tokens={T}` (stdout).
7. Закрой child (kill/wait), таймаут на весь обмен: разумный (напр. 10s на чтение) через `std::io` + thread или полагайся на fixture — для простоты: чтение line-based с `BufRead::read_line`, полагаясь на EOF/child exit; тестовые фикстуры отвечают мгновенно. Если нужен таймаут — `wait_timeout` без новых крейтов НЕЛЬЗЯ; допустимо читать без таймаута в 07a (timeout 10s доделаем в audit rmcp path, там tokio).

### Wiring cli.rs

`ModuleCommand::Measure(NameArgs)` → resolve name (None → Err «имя обязательно в M0»), прочитать command из ModulesFile, вызвать measure_module(cwd, name, command), печатать отчёт, exit 0/1. Match-arm — в КОНЕЦ существующего match по ModuleCommand, чужие arms не трогай (merge-протокол в M0-index). lib.rs уже имеет cmd_measure с 06 — не трогай (forbidden).

## Тесты (добавить в cmd_measure.rs; старые НЕ ослаблять)

1. Все тесты packet-06 зелёные (parse, bytes, upsert, pinning).
2. **Fake MCP server**: unit-тест клиента через подмену transport недоступна (sync struct). Минимально: вынеси `run_session(reader, writer, tpb) -> Result<SessionOut>` работающую над `impl BufRead/Write` — тесты крутят `Cursor<Vec<u8>>` с предзаписанным ответом FIXTURE_TOOLS из packet-06 (тот же константный фикстур — не изобретай новый). Запись lock через tempdir — настоящая.
   - Схема: `run_session(r, w, tpb) -> Result<SessionOut>` где SessionOut { schema_bytes, tokens }; measure_module = spawn process + run_session над pipe'ов.
   - Для spawn-теста: `/bin/sh`-скрипт в tempdir, chmod +x, content ровно (не адаптируй):

```sh
#!/bin/sh
while IFS= read -r line; do
  case "$line" in
    *'"method":"initialize"'*)
      printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"fake-mcp","version":"0.0.1"}}}' ;;
    *notifications/initialized*) ;;
    *'"method":"tools/list"'*)
      printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"t1","inputSchema":{"type":"object"}}]}}' ;;
  esac
done
```

   ids 1/2 совпадают с порядком клиента из спецификации выше. sh-скрипт в tempdir — ОК (не новый крейт).
3. Интеграционный стиль: tempdir modules.toml (polyforge), fake script как `command` (chmod +x), measure_module → отчёт B>0, T==round(B*tpb), lock содержит имя, второй вызов upsert не дублирует. Spawn, если бинарь не стартовал (io::Error NotFound) → MeasureError::Spawn (cli exit 1), без паники.
4. CLI: run(["stellwerk","module","measure","polyforge"]) с cwd=tempdir → exit 0, stdout содержит `measure polyforge:`; без модуля → exit 1.

## Приёмка

- `cargo test -p stellwerk-registry measure` → 0.
- clippy -D warnings чисто.
- НЕ запускай реальный polyforge-mcp (это 07b).

## Evidence

- GREEN вывод; PROOF evidence_verify(cmd='cargo test -p stellwerk-registry measure'); GATE; STATUS/FILES/TEST/EVIDENCE.
