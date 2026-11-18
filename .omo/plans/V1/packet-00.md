---
id: 00
milestone: V1
status: GREEN
depends_on: []
files_allowed: [docs/p00-baseline.md, crates/stellwerk-cli/tests/p00_baseline.rs]
acceptance: cargo test -p stellwerk-cli --test p00_baseline
---

# 00 — Измеренная база P00

## Цель
База из шести измеренных команд закреплена документом и тестом-стражем, потому что база без проверяемого артефакта неотличима от пересказа состояния.

## Обоснование (измерено)
- `justfile:5` — цель `fmt` запускает `cargo fmt --all -- --check`, то есть exit-код снимается одной командой.
- `justfile:8` — цель `lint` запускает `cargo clippy --workspace --all-targets -- -D warnings`.
- `docs/PI_1_0_2_PROBE.md:10` — на машине установлена Pi 0.87.0, поэтому версия в базе фиксируется измерением, а не предположением.

## Шаг 1 — падающий тест
`crates/stellwerk-cli/tests/p00_baseline.rs`, тест `baseline_lists_every_measured_command`: читает `docs/p00-baseline.md` и требует по строке на каждую из шести команд (`fmt`, `clippy`, `cargo test`, `plan-lint`, `replay inventory`, `doctor`) вместе с exit-кодом; до создания документа файл отсутствует, поэтому тест падает на открытии и на первой пропущенной строке.

## Приёмка
cargo test -p stellwerk-cli --test p00_baseline
Ожидается PASS.

## Запрещено
- нельзя вписывать в документ числа, которых не было в измеренном прогоне.
- запрещено объявлять `plan-lint` зелёным: его exit-код на базе — 1.
- нельзя превращать тест-страж в проверку наличия файла без сверки строк.
