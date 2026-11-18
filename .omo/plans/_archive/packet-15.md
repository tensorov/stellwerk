# packet-15 — CLI: честный `verify`, без прямого promotion

plan_id: R1
status: предложен
goal: RED — CLI не может создать `Verified`/`Validated` без запуска allowlisted-инструмента; честный путь — новая команда `pf verify`.
files_allowed: [crates/polyforge-cli/src/main.rs, README.md, docs/integrations/langfuse.md]
max_diff_lines: 180
test: cargo test -p polyforge-cli
acceptance: >
  `pf append` принимает только `model_claim` и `discrepancy`. `tool_attestation`,
  `eval_attestation`, `validation` удалены из `parse_kind` и `cmd_append` и
  возвращают ошибку с указанием использовать `pf verify`. Новая команда:
  `pf verify <task_id> --claim <seq> --tool "<canonical-name>" [-- <args>...]`.
  Она вызывает `polyforge_toolrunner::lookup`, затем
  `verify_and_append(&mut ledger, task_id, claim_id, &tool, &args)`. На успехе
  печатает `verified entry <seq>`; на ошибке инструмента печатает `ToolFailed`
  и возвращает exit 1, не создавая `Verified`. Нельзя передать произвольный
  payload как доказательство: payload больше не принимается для
  promoted-состояний. README first-run: `polyforge-cli append model_claim ...` →
  `polyforge-cli verify ... --tool "cargo --version"` (пример), без `append
  tool_attestation` и `append validation`. `docs/integrations/langfuse.md`
  больше не показывает `append tool_attestation`; показывает `pf verify`.
  Unit-тесты в `main.rs`: `append tool_attestation` → error, ledger не изменился;
  `append validation` → error, ledger не изменился; `verify` с `cargo --version`
  → `Verified`, gate проходит; `verify` с несуществующим claim → error, ledger
  не изменился.
forbidden: [оставлять любой путь, который создаёт promoted-состояние из строки payload, добавлять `--force`/`--trust-me`/`PF_SKIP_TOOL`, менять `evidence.rs`/`promote` или ослаблять state machine, добавлять зависимость, оставлять README-пример с удалённой командой]
depends_on: []
mutation_threshold: 80
data_class: private
lane: runtime

## Контекст

MCP уже запрещает модели создавать promoted-состояния, но CLI — второй вход. Если у
модели есть `bash` и approve, она обходит MCP через `append tool_attestation` или
`append validation`. Удаление прямого promotion и добавление `pf verify` закрывает
этот вход, не ослабляя операторский workflow.

## Что сделать

1. Оставить в `parse_kind`/`cmd_append` только `model_claim` и `discrepancy`.
2. Для `tool_attestation`, `eval_attestation`, `validation` возвращать ошибку с
   указанием использовать `pf verify`.
3. Добавить `pf verify <task_id> --claim <seq> --tool "<canonical-name>" [-- <args>...]`.
   Внутри: `lookup(tool)` + `verify_and_append(...)`; не строить `Tool` вручную.
4. На успехе печатать `verified entry <seq>`; на `ToolFailed` — exit 1 и не создавать
   `Verified`.
5. Обновить README first-run и `docs/integrations/langfuse.md`: только `append
   model_claim` и `pf verify`, без удалённых команд.

## Проверка

RED: unit-тесты в `main.rs` на удалённые команды и на `verify` с `cargo --version`
должны падать до правки. GREEN: `cargo test -p polyforge-cli`. Обязательны проверки:
ledger не меняется при отказе; `verify` с несуществующим claim не пишет запись.
