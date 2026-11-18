---
id: i02
milestone: V1
status: GREEN
depends_on: [i01]
files_allowed: [.github/workflows/polyforge.yml, crates/stellwerk-cli/tests/ci_gate.rs]
acceptance: cargo test -p stellwerk-cli --test ci_gate
---

# i02 — contract-change: Негативный тест CI-гейта

**label:** `contract-change` — owner approved 2026-11-03 for `.github/workflows/polyforge.yml`.

## Цель
Бандл без аттестации даёт exit 1 с названной причиной, и тест проверяет код возврата, потому что мягкая проверка подстроки проходит на гейте, который ничего не запретил.

## Обоснование (измерено)
- `.github/workflows/polyforge.yml:9` — шаг гейта до сих пор печатает заглушку, поэтому негативный тест обязан опираться на код возврата скрипта, а не на текст YAML.

## Шаг 1 — падающий тест
`crates/stellwerk-cli/tests/ci_gate.rs`, тест `unattested_bundle_returns_exit_one`: подставляет фиктивный `polyforge-mcp`, отвечающий `{"result":{"ok":false,"state":"MissingToolAttestation"}}`, и требует exit 1 и печать `MissingToolAttestation`; при `{"result":{"ok":true,"state":"Verified"}}` требует exit 0. До реализации гейта скрипт отсутствует, и тест падает на отсутствии exit-кода.

## Приёмка
cargo test -p stellwerk-cli --test ci_gate
Ожидается PASS.

## Запрещено
- нельзя проверять только подстроку в YAML вместо поведения.
- запрещено считать exit 0 при `ok:false`.
- нельзя делать негативный тест мягким через `contains` вместо кода возврата.
