---
id: v02
milestone: V1
status: GREEN
depends_on: [c02]
files_allowed: [crates/stellwerk-cli/tests/vertical_v2.rs]
acceptance: cargo test -p stellwerk-cli --test vertical_v2
---

# v02 — Вертикальное доказательство отказа

## Цель
Запрещённый вызов даёт `Deny` или `Suspended`, не создаёт кандидат и оставляет evidence с отказом, потому что отказ, записанный как пустота, неотличим от прогона без решения.

## Обоснование (измерено)
- `crates/stellwerk-gateway/src/gate.rs:37` — здесь формируется вердикт отказа, который вертикальный тест обязан довести до evidence.
- `crates/stellwerk-cli/src/build_runner.rs:82` — сборка кандидата начинается в workspace, поэтому отказ должен остановить её до появления файлов.

## Шаг 1 — падающий тест
`crates/stellwerk-cli/tests/vertical_v2.rs`, тест `refused_call_leaves_no_candidate_and_evidence`: требует, чтобы после запрещённого вызова кандидата не было, а evidence содержал названный отказ; до связывания отказа с evidence прогон оставляет пустой след, и тест падает на отсутствии записи об отказе.

## Приёмка
cargo test -p stellwerk-cli --test vertical_v2
Ожидается PASS.

## Запрещено
- нельзя трактовать отсутствие ответа как разрешение.
- запрещено создавать кандидат при отказе.
- нельзя записывать отказ как пустоту без evidence.
