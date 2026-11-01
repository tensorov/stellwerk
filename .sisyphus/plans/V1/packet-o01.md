---
id: o01
milestone: V1
status: GREEN
depends_on: [03, c02]
files_allowed: [crates/stellwerk-task/src/evidence.rs, crates/stellwerk-task/tests/evidence.rs]
overlap_waiver: C9 — N1/07 — `evidence.rs`, последовательное исполнение
acceptance: cargo test -p stellwerk-task --test evidence
---

# o01 — Порядок evidence `ModelClaim → ToolAttestation → Validation`

## Цель
`Validated` требует реального `ToolAttestation`, а каждое звено хранит свой digest и команду, потому что цепочка, собранная из слов модели, доказывает только то, что модель себя объявила готовой.

## Обоснование (измерено)
- `crates/stellwerk-task/src/evidence.rs:61` — здесь начинается запись `ModelClaim`, и команда у неё пуста по замыслу.
- `crates/stellwerk-task/src/evidence.rs:189` — здесь заканчивается построение цепочки, где порядок звеньев обязан проверяться явно.

## Шаг 1 — падающий тест
`crates/stellwerk-task/tests/evidence.rs`, тест `validated_requires_tool_attestation`: утверждает, что пропуск звена даёт отказ, а цепочка из трёх звеньев даёт `Validated`; до фиксации порядка цепочка без `ToolAttestation` доходит до `Validated`, и тест падает на этом ложном успехе.

## Приёмка
cargo test -p stellwerk-task --test evidence
Ожидается PASS.

## Запрещено
- нельзя выводить аттестацию из текста модели.
- запрещено принимать `Validated` без `ToolAttestation`.
- нельзя хранить звено без digest и команды.
