---
id: v01
milestone: V1
status: GREEN
depends_on: [a02b, r04]
files_allowed: [crates/stellwerk-cli/tests/vertical_v1.rs]
acceptance: cargo test -p stellwerk-cli --test vertical_v1
---

# v01 — Вертикальное доказательство host-executed edit

## Цель
Сквозной прогон доказывает, что хост авторизует `edit` с digest и сам применяет запись, потому что успех, собранный из ответа Pi, не отличает исполнение хостом от исполнения внутри Pi. Пакет остаётся test-only: перенос эффекта в хост сделан отдельным `a02b`, чтобы `v01` не расширял `files_allowed` до продакшн-кода.

## Обоснование (измерено)
- `crates/stellwerk-task/pi/stellwerk_transport.ts:128-140` — до `a02b` extension писал файл сам через `writeFileSync`; после `a02b` этот путь удалён, и хост применяет `AuthorizedCall`.
- `crates/stellwerk-cli/src/build_runner.rs:858` — поток гейта после `a02b` получает корень workspace и вызывает `apply_gate_decision`; `v01` обязан пройти именно через этот путь.
- `crates/stellwerk-cli/src/build_runner.rs:456` — исполнение вызова не должно обходить авторизованный путь.

## Шаг 1 — падающий тест
`crates/stellwerk-cli/tests/vertical_v1.rs`, тест `host_executed_edit_end_to_end`: требует цепочки «Pi просит `edit` → хост авторизует с digest → файл изменён → digest после записи равен авторизованному → кандидат содержит файл»; до связывания шагов запись не доходит до хоста, digest после записи не с чем сравнить, и тест падает на первом же отсутствующем звене.

## Приёмка
cargo test -p stellwerk-cli --test vertical_v1
Ожидается PASS.

## Запрещено
- нельзя утверждать успех без реального прогона Pi.
- запрещено подменять Pi фикстурой.
- нельзя считать запись успешной без сверки digest после записи.
- нельзя править `transport.ts`/`build_runner.rs` в этом пакете: это область `a02b`.
