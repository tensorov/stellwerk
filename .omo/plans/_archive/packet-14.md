# packet-14 — MCP gate требует явные required-состояния

plan_id: R1
status: предложен
goal: RED — `gate_evaluate` без `required` или с `required: []` возвращает ошибку, а не `passed: true`.
files_allowed: [crates/polyforge-mcp/src/server.rs, crates/polyforge-mcp/tests/server_tools.rs, crates/polyforge-mcp/tests/client_smoke.rs]
max_diff_lines: 120
test: cargo test -p polyforge-mcp
acceptance: >
  У `GateEvaluateParams.required` нет `#[serde(default)]`; отсутствие поля — ошибка
  десериализации, а не пустой Vec. `gate_evaluate` с `required: []` возвращает
  `invalid_params` с текстом `required must name at least one evidence state`.
  `required: ["Verified"]` на одном `ModelClaim` даёт `passed: false`,
  `missing: ["Verified"]`. После `evidence_verify` тот же вызов даёт `passed: true`.
  `client_smoke` больше не проверяет «пустой required проходит»; он передаёт
  `["Verified"]`. Никакого default-состояния (`Verified`/`Validated`) не подставляется.
forbidden: [подставлять `required.unwrap_or(vec![Verified])`, принимать пустой `required` как «все состояния», менять семантику `gate.rs` или `evidence_append`, добавлять зависимость]
depends_on: []
mutation_threshold: 80
data_class: private
lane: runtime

## Контекст

`gate.rs` корректен, но MCP-поверхность позволяет обойти требование, не называя его.
`GateEvaluateParams.required` имеет `#[serde(default)]` и `Default`, а `gate_evaluate`
передаёт пустой Vec в `evaluate_complete`. В результате голый `ModelClaim` проходит
gate без единого `Verified`. Это отдельный вход, поэтому пакет отдельный от B-16.

## Что сделать

1. Убрать `#[serde(default)]` и `Default` у `GateEvaluateParams.required`; отсутствие
   поля должно быть ошибкой десериализации.
2. В `gate_evaluate` явно отвергать пустой `required` с `invalid_params` и текстом
   `required must name at least one evidence state`.
3. Обновить `server_tools` и `client_smoke`: оба передают `["Verified"]`; ни один не
   проверяет «пустой required проходит».
4. Сохранить существующую семантику `evaluate_complete` и `gate.rs` без изменений.

## Проверка

RED: добавить тест `gate_evaluate` с `required: []` и тест на `ModelClaim`; оба должны
падать до правки. GREEN: `cargo test -p polyforge-mcp`. Обязательны позитивный контроль
(после `evidence_verify` gate проходит) и негативный (пустой `required` отвергнут).
