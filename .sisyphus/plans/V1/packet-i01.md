---
id: i01
milestone: V1
status: GREEN
depends_on: [v03]
files_allowed: [.github/workflows/polyforge.yml, scripts/integrator/gate.sh, crates/stellwerk-cli/tests/ci_gate.rs]
acceptance: cargo test -p stellwerk-cli --test ci_gate
---

# i01 — contract-change: CI-гейт PolyForge вместо заглушки

**label:** `contract-change` — owner approved 2026-11-03 for `.github/workflows/polyforge.yml`.

## Цель
CI-гейт поднимает `polyforge-mcp` по stdio и зовёт `gate_evaluate`, а при отсутствии команды падает с exit 2 и `MissingPolyForge`, потому что гейт, который печатает `TODO`, не отличает проверенный бандл от непроверенного.

## Обоснование (измерено)
- `modules.toml:14` — модуль `polyforge` объявлен с `command = "polyforge-mcp"`, и гейт берёт команду отсюда, если `POLYFORGE_CMD` не задан.
- `crates/stellwerk-registry/src/cmd_measure.rs:207` — здесь реализован рукопожатный `run_session` по stdio, который служит образцом протокола для `gate.sh`.
- `.github/workflows/polyforge.yml:9` — шаг гейта сейчас печатает `TODO`, поэтому его нужно заменить вызовом скрипта.

## Шаг 1 — падающий тест
`crates/stellwerk-cli/tests/ci_gate.rs`, тест `gate_script_fails_closed`: требует поведения (`ok:false` → exit 1, `ok:true` → exit 0), fail-closed (`POLYFORGE_CMD=/nonexistent` → exit 2 и `MissingPolyForge`) и smoke (YAML содержит `scripts/integrator/gate.sh` и не содержит прощения ошибки в job `gate`); до появления скрипта тест падает на отсутствии файла и на старом `echo TODO`.

## Приёмка
cargo test -p stellwerk-cli --test ci_gate
Ожидается PASS.

## Запрещено
- нельзя утверждать гейт по наличию строки в YAML.
- запрещено оставлять `continue-on-error` на шаге гейта.
- нельзя считать exit 2 успехом: отсутствие PolyForge — это провал, а не разрешение.
