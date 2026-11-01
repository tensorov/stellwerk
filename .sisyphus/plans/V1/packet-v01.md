---
id: v01
milestone: V1
status: GREEN
depends_on: [a02, r04]
files_allowed: [crates/stellwerk-cli/tests/vertical_v1.rs]
acceptance: cargo test -p stellwerk-cli --test vertical_v1
---

# v01 — Вертикальное доказательство host-executed edit

## Цель
Сквозной прогон доказывает, что хост авторизует `edit` с digest и сам применяет запись, потому что успех, собранный из ответа Pi, не отличает исполнение хостом от исполнения внутри Pi.

## Обоснование (измерено)
- `crates/stellwerk-cli/src/build_runner.rs:456` — исполнение вызова сейчас идёт через `worker.exec("workspace", ...)`, и вертикальный тест обязан пройти через авторизованный путь.
- `crates/stellwerk-task/pi/stellwerk_transport.ts:259` — транспорт шлёт вызов в сокет, поэтому вертикальный тест может наблюдать именно хостовый эффект.

## Шаг 1 — падающий тест
`crates/stellwerk-cli/tests/vertical_v1.rs`, тест `host_executed_edit_end_to_end`: требует цепочки «Pi просит `edit` → хост авторизует с digest → файл изменён → digest после записи равен авторизованному → кандидат содержит файл»; до связывания шагов запись не доходит до хоста, digest после записи не с чем сравнить, и тест падает на первом же отсутствующем звене.

## Приёмка
cargo test -p stellwerk-cli --test vertical_v1
Ожидается PASS.

## Запрещено
- нельзя утверждать успех без реального прогона Pi.
- запрещено подменять Pi фикстурой.
- нельзя считать запись успешной без сверки digest после записи.
