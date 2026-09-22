# packet-06
plan_id: M0
goal: measure RED — Lock-типы, разбор tools/list JSON, пиннинг-тест байты→токены 813±10% (заглушки, тесты падают).
files_allowed: [crates/stellwerk-registry/src/lib.rs, crates/stellwerk-registry/src/cmd_measure.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-registry measure
acceptance: RED: bash -c 'cargo build -p stellwerk-registry && cargo test -p stellwerk-registry measure --no-run && ! cargo test -p stellwerk-registry measure' → exit 0.
forbidden: [cargo add, правка корневого Cargo.toml, правка modules.lock (реального репо), правка modules.toml, правка contract.rs, правка cli.rs, сеть, spawn реального MCP, #[ignore], #[allow], исправление теста]
depends_on: [packet-04]

## RED-пакет — правила

DO NOT fix/red-green the test inside a RED packet. Зелёным сделает packet-07a (та же ветка lane-measure). Реального polyforge-mcp в этом packet НЕТ — только фикстуры JSON и tempdir.

## Контекст

- measure: tools/list JSON-RPC от MCP-сервера → байты compact-JSON схем (inputSchema+outputSchema по каждому тулу) → `tokens_for(bytes, calibration.tokens_per_byte)` → запись `context_cost_tokens` в modules.lock (tempdir в тестах; реальный lock — binary only, Q6).
- modules.lock формат: `version = 1` + по одному блоку `[<name>]` на модуль, сортировка по имени (BTreeMap), TOML. Комментарий-заголовок репо «Генерируется… Руками не править» — при записи через toml crate он не сохраняется; допустимо перезаписать tempdir-файл целиком (в реальном репо lock пишет только бинарь в 07b/final acceptance).
- Calibration читается из modules.toml (`ModulesFile`); formula — `model::tokens_for`.
- JSON tools/list (JSON-RPC 2.0): `{"jsonrpc":"2.0","id":1,"result":{"tools":[{"name":"...","inputSchema":{...},"outputSchema":{...}?}, ...]}}`.

## Что сделать

1. lib.rs: append `pub mod cmd_measure;` ПОСЛЕ последней существующей `pub mod` (не в середину; merge-протокол в M0-index — при слиянии lanes оставляй обе строки). cli.rs НЕ трогать.
2. cmd_measure.rs — типы + заглушки (компилируются, возвращают пустое/0/Err):
   - `LockModule { context_cost_tokens: u64 }` + serde.
   - `LockFile { version: u32, modules: BTreeMap<String, LockModule> }` — serialize/deserialize как modules.lock (flatten map на top-level: `version` + `[name]` секции).
     Реализация: `#[serde(flatten)] modules: BTreeMap<String, LockModule>` + поле `version` default 1.
   - `parse_tools_list_response(json: &str) -> Result<Vec<ToolSchema>, MeasureError>` — ToolSchema { name, schema_json: String } где schema_json = compact (`serde_json::to_string`) конкатенация inputSchema и outputSchema (если outputSchema есть — `{"input":...,"output":...}`; если outputSchema отсутствует или null — только input). inputSchema отсутствует или null → Err(InvalidTool). RED: `Err(NotImplemented)`.
   - `schema_bytes(tools: &[ToolSchema]) -> u64` — сумма `.schema_json.len() as u64`. RED: `0`.
   - `upsert_lock(existing: &str, name: &str, tokens: u64) -> Result<String, MeasureError>` — распарсить existing, вставить/обновить модуль, отсортировать, сериализовать. RED: `Err(NotImplemented)`.
   - `measure_tokens(schema_b: u64, tpb: f64) -> u64` — делегирует `model::tokens_for`. RED можно оставить рабочей (уже зелёная в 02) — не обязательно ломать; падение обеспечат остальные.

Никакого `std::process` в этом packet — spawn/inject приходит в 07a.

## Тесты (filter `measure`, в cmd_measure.rs tests; путь содержит `measure`)

Верbatim (для Momus, дословно в комментарии теста и в тексте packet):

> 813 ±10% — это context_input-дельта измерения OpenCode (docs/CONTEXT_BUDGET.md), включая оверхед форматирования тулов, НЕ чистые байты схем; тест — pinning round-trip (bytes × tokens_per_byte ≈ 813), не проверка правильности коэффициента.

1. FIXTURE (полный, единственный для всех тестов measure в 06/07a) — tools/list response, 4 тула как у polyforge-mcp, ключи внутри схем в alphabetical order (serde_json Value без preserve_order пересериализует в такой же порядке):

```rust
const FIXTURE_TOOLS: &str = r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[
{"name":"evidence_append","inputSchema":{"properties":{"kind":{"type":"string"},"payload":{"type":"string"}},"required":["kind","payload"],"type":"object"}},
{"name":"evidence_verify","inputSchema":{"properties":{"claim_id":{"type":"integer"},"task_id":{"type":"string"}},"required":["claim_id","task_id"],"type":"object"}},
{"name":"gate_evaluate","inputSchema":{"properties":{"required":{"items":{"type":"string"},"type":"array"},"task_id":{"type":"string"}},"required":["task_id"],"type":"object"}},
{"name":"gate_report","inputSchema":{"properties":{"task_id":{"type":"string"}},"required":["task_id"],"type":"object"},"outputSchema":{"properties":{"ok":{"type":"boolean"}},"type":"object"}}
]}}"#;
// Ожидаемые schema_json (ровно эти строки, alphabetical keys):
const SJ0: &str = r#"{"properties":{"kind":{"type":"string"},"payload":{"type":"string"}},"required":["kind","payload"],"type":"object"}"#;
const SJ1: &str = r#"{"properties":{"claim_id":{"type":"integer"},"task_id":{"type":"string"}},"required":["claim_id","task_id"],"type":"object"}"#;
const SJ2: &str = r#"{"properties":{"required":{"items":{"type":"string"},"type":"array"},"task_id":{"type":"string"}},"required":["task_id"],"type":"object"}"#;
const SJ3: &str = r#"{"input":{"properties":{"task_id":{"type":"string"}},"required":["task_id"],"type":"object"},"output":{"properties":{"ok":{"type":"boolean"}},"type":"object"}}"#;
```

Тест: parse(FIXTURE_TOOLS) → Ok; каждый tools[i].schema_json == SJ_i; schema_bytes == (SJ0.len()+SJ1.len()+SJ2.len()+SJ3.len()) as u64. Ничего не вычисляй «руками по-своему» — только эти константы. (RED падает.)
2. Невалидный JSON / отсутствующий result.tools → Err.
3. Пиннинг: `let tpb = 813.0 / B as f64; let t = measure_tokens(B, tpb);` assert t в диапазоне 732..=894 (813±10%). Для B из фикстуры. (Может быть зелёным если measure_tokens жив — допустимо; основной RED — п.1 и п.4.)
4. upsert_lock: вход `version = 1\n` → выход содержит `[polyforge]`, `context_cost_tokens = 813`, `version = 1`; повторный upsert другого имени — оба блока, сортировка alpha (aaa перед zzz); обновление существующего имени заменяет tokens, не дублирует блок. (RED падает.)
5. upsert_lock на мусоре → Err.
6. LockFile round-trip: to_string → from_str равны для 2 модулей.

## Evidence

- RED wrapper → 0, вывод падающих.
- PROOF/GATE — после 07a, не здесь.
- STATUS/FILES/TEST/EVIDENCE.
