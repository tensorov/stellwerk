# M0-index — карта packets (для plan-lint и раздачи lanes)

> Хранение: контент packets — в `.omo/plans/M0/`. Перед `just plan-lint` и раздачей воркерам скопировать в `.sisyphus/plans/M0/` и положить этот индекс как `.sisyphus/plans/M0-index.md` (запрет prometheus-md-only на запись в `.sisyphus/` — снять или скопировать вручную).
> Воркер stellwerk-micro видит ТОЛЬКО свой packet-NN.md. Формат строго по docs/PROMETHEUS_PROMPT.md.

## Карта packet → lane → depends_on

| packet | lane | depends_on | test (cargo test -p stellwerk-registry …) |
| --- | --- | --- | --- |
| packet-01 | m0/model | — | model (RED) |
| packet-02 | m0/model | 01 | model (GREEN) |
| packet-03 | m0/cli | 02 | cli (RED) |
| packet-04 | m0/cli | 03 | cli (GREEN) |
| packet-05 | m0/add | 04 | add (RED+GREEN) |
| packet-06 | m0/measure | 04 | measure (RED) |
| packet-07a | m0/measure | 06 | measure (GREEN) |
| packet-07b | m0/measure | 07a | measure (калибровка) |
| packet-08 | m0/audit | 04 | audit (RED + rmcp) |
| packet-09 | m0/audit | 08 | audit (GREEN) |
| packet-10 | m0/update | 07b, 09 | update (RED) |
| packet-11 | m0/update | 10 | update (GREEN) |

Волны: model (01→02) → cli (03→04) → параллельно add (05) | measure (06→07a→07b) | audit (08→09) → update (10→11). Merge в main только после GREEN lane; RED без GREEN не мержится.

## Merge параллельных lanes (обязательно при слиянии веток add|measure|audit)

Ветки правят ОДНИ и те же точки: `lib.rs` (append `pub mod cmd_*;` строкой ПОСЛЕ последней существующей `pub mod`, не в середину) и `cli.rs` (новый match-arm — в КОНЕЦ соответствующего `match`, не меняй чужие arms). При merge в main ожидается тривиальный conflict «both added»: оставляй ОБЕ добавленные строки (все `pub mod`, все match-arm), не переупорядочивай и не удаляй чужие правки. Пакеты 05/06/07a/08/09 несут эти anchor-инструкции внутри.

## packet-01
lane m0/model, RED, files: src/lib.rs, src/model.rs — типы + формула tokens_for, падающие тесты model.

## packet-02
lane m0/model, GREEN, files: src/model.rs — реализация from_toml_str/load/tokens_for.

## packet-03
lane m0/cli, RED, files: src/lib.rs, src/cli.rs — clap-скелет module add|measure|audit|update, падающие тесты cli.

## packet-04
lane m0/cli, GREEN, files: src/cli.rs, src/main.rs — parse_from + run() exit 0/1/2, dispatch на placeholder.

## packet-05
lane m0/add, files: src/cmd_add.rs, src/lib.rs, src/cli.rs — append в modules.toml, валидация source, дубликат → ошибка; cargo test add.

## packet-06
lane m0/measure, RED, files: src/lib.rs, src/cmd_measure.rs — Lock-типы, parse_tools_list_response, пиннинг-тест 813±10%; cargo test measure.

## packet-07a
lane m0/measure, GREEN, files: src/cmd_measure.rs, src/cli.rs — std::process JSON-RPC stdio (пошаговый обмен), tools/list → bytes → tokens → modules.lock (tempdir); cargo test measure.

## packet-07b
lane m0/measure, files: modules.toml, modules.lock — калибровка: rename bytes_per_token→tokens_per_byte, реальный polyforge-mcp, context_cost_tokens ∈ 732..=894; cargo test measure (регресс).

## packet-08
lane m0/audit, RED, files: crates/stellwerk-registry/Cargo.toml (единственная правка Cargo.toml в M0: rmcp = { workspace = true }), src/lib.rs, src/cmd_audit.rs — trait-шов McpProbe + тесты с фейком; cargo test audit.

## packet-09
lane m0/audit, GREEN, files: src/cmd_audit.rs, src/cli.rs — пошаговый stdio JSON-RPC-клиент McpProbe + обязательный импорт rmcp::model (dep из 08 live), tools/list exact-set, tools/call {}, timeout 10s, exit 0/1; cargo test audit.

## packet-10
lane m0/update, RED, files: src/lib.rs, src/cmd_update.rs — CommandRunner trait, STELLWERK_GH_BIN, pending-патч .stellwerk/pending/; cargo test update.

## packet-11
lane m0/update, GREEN, files: src/cmd_update.rs, src/cli.rs — полный flow fetch→build→audit→measure→gh pr create (никогда merge); cargo test update.

## DEFERRED

Всё не-M0 — deferred, не планировать в этом пакете: audit для kind=skill (SKILL.md); точный токенизатор qwen (measure = байты × калибровка, calibration-коэффициент; без hf/huggingface); stellwerk-gateway, stellwerk-bus, stellwerk-sync, stellwerk-nag, stellwerk-hooks; kanban / Hermes / OpenResearch / CRM / Obsidian-проекция / IoT / TTS — deferred, см. docs/DEFERRED.md.

## Калибровка (calibration)

packet-07b — обязательная калибровка на реальном polyforge-mcp: schema_bytes B, tokens_per_byte = 813/B (6 знаков), measured_at ISO8601, запись в modules.lock. 813 ±10% = context_input-дельта OpenCode (docs/CONTEXT_BUDGET.md), НЕ чистые байты схем; тест — pinning round-trip, не correctness коэффициента.

## Ограничения пакетов

каждый packet ≤3 файла, ≤200 строк diff, ≤4k токенов; зависимости только из workspace allowlist; прямое добавление крейтов через CLI-команды пакетного менеджера запрещено; новый крейт → BLOCKED.
