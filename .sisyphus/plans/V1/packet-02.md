---
id: 02
milestone: V1
status: GREEN
depends_on: [00]
files_allowed: [docs/adr/0021-single-execution-profile.md, crates/stellwerk-task/src/execution.rs, crates/stellwerk-task/tests/execution.rs]
overlap_waiver: C9 — N1/02 — `execution.rs`, `tests/execution.rs`, последовательное исполнение
acceptance: cargo test -p stellwerk-task --test execution
---

# 02 — Один профиль исполнения `pi`

## Цель
Единственный разрешённый профиль — `pi`, и любой другой профиль отвергается названной причиной, потому что два живых профиля делают недоказуемым утверждение «исполнялся именно авторизованный движок».

## Обоснование (измерено)
- `crates/stellwerk-task/src/execution.rs:188` — `resolve` сверяет профиль с `ALLOWED_PROFILES`, и сейчас там живёт `fixture-engine`.
- `crates/stellwerk-task/src/execution.rs:25` — список `ALLOWED_PROFILES` задан рядом с типом профиля, поэтому убирать второй профиль нужно здесь, а не в вызывающем коде.
- `crates/stellwerk-task/tests/execution.rs:174` — существующее ожидание разрешает профиль `fixture-engine` и закрепляет ровно отменяемое поведение.

## Шаг 1 — падающий тест
`crates/stellwerk-task/tests/execution.rs`, тест `rejects_every_profile_except_pi`: утверждает, что `resolve` с профилем `fixture-engine` даёт ошибку, а с `pi` — успех; до правки `fixture-engine` разрешён, поэтому тест падает на первом же утверждении, а строка 174 закрепляет старое разрешение.

## Приёмка
cargo test -p stellwerk-task --test execution
Ожидается PASS.

## Запрещено
- нельзя оставлять `fixture-engine` в `ALLOWED_PROFILES`.
- запрещено добавлять дефолт профиля: отсутствие профиля — отдельный отказ, а не `pi`.
- нельзя оставлять тест, закрепляющий разрешение `fixture-engine`.
