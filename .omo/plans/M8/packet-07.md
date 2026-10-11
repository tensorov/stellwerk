# packet-07 — gateway и runner перестают считать SHA-256 сами

plan_id: M8
status: предложен
goal: GREEN — в stellwerk-gateway нет приватной копии sha256_hex, инлайн в runner заменён вызовом core
files_allowed: [crates/stellwerk-gateway/src/session.rs, crates/stellwerk-task/src/runner.rs]
max_diff_lines: 200
test: cargo mutants --package stellwerk-gateway --jobs 18 --timeout 300
acceptance: grep по gateway/src не находит определения sha256_hex; gateway baseline не ухудшился; runner.rs не содержит format!("{:x}", ..)
forbidden: [трогать shaping.rs (побайтовый hex, другая семантика, не в объёме), трогать task/evidence.rs и task/approval.rs (пакет 06), переписать runner.rs инлайн шире, чем замена одной строки]
depends_on: [04, 05]
mutation_threshold: 80
data_class: private
lane: runtime

## Контекст

Продолжение M8-06. Здесь два разных случая, и их нельзя смешивать.

**Случай 1 — приватная копия-функция.**
`crates/stellwerk-gateway/src/session.rs:384`, используется в пяти местах:
строки 97, 129, 159, 258, 286. Функция удаляется, добавляется импорт core.

**Случай 2 — инлайн без функции.**
`crates/stellwerk-task/src/runner.rs:371`:
    let digest = format!("{:x}", Sha256::digest(outcome.output.as_bytes()));
Это не копия функции, а вызов в месте использования. Заменяется ОДНОЙ строкой
на `sha256_hex(outcome.output.as_bytes())`. Больше в runner.rs ничего не трогай:
пакет не про рефакторинг runner, он про единственный источник хеша.

## Чего этот пакет НЕ делает

`crates/stellwerk-gateway/src/shaping.rs:132` — это **побайтовый hex**, а не
`format!("{:x}", ..)`. Другая реализация со своей семантикой (длина и регистр
проверяются её собственными тестами). Приводить её к core в этом пакете НЕЛЬЗЯ:
это отдельное решение с отдельным обоснованием.

Если при чтении кода покажется, что побайтовый вариант можно безопасно заменить —
не заменяй. Заведи находку и верни её владельцу.

## Приёмка

- `grep -rn "fn sha256_hex" crates/stellwerk-gateway/src/` пусто.
- `grep -n 'format!("{:x}"' crates/stellwerk-task/src/runner.rs` пусто.
- `cargo mutants --package stellwerk-gateway`: сравни с baseline M8-01
  (269 total / 14 missed / 0 timeout). missed не должен вырасти.
- `cargo mutants --package stellwerk-task` тоже прогнать: пакет меняет и его.
- clippy по обоим крейтам чист.

Таймауты — утверждение о машине (ADR-0014): load < 40, один прогон за раз,
артефакты в `~/gits/verify/` вне репозитория.
