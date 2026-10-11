# packet-06 — task перестаёт держать собственные копии sha256_hex

plan_id: M8
status: предложен
goal: GREEN — stellwerk-task не содержит ни одного собственного определения sha256_hex, оба приватных вызывают core
files_allowed: [crates/stellwerk-task/src/evidence.rs, crates/stellwerk-task/src/approval.rs]
max_diff_lines: 200
test: cargo mutants --package stellwerk-task --jobs 18 --timeout 300
acceptance: grep по task/src не находит определения sha256_hex; baseline не ухудшился ни по одному показателю; cargo clippy -p stellwerk-task --all-targets чист
forbidden: [трогать gateway/session.rs и task/runner.rs (это пакет 07), менять тело format!("{:x}", ..) на другое, удалять покрытие, «улучшать» хеш-вызов попутно]
depends_on: [04, 05]
mutation_threshold: 80
data_class: private
lane: runtime

## Контекст

M8-04 создал единственное определение в `stellwerk-core`. Этот пакет убирает две
приватные копии из `stellwerk-task`:

- `crates/stellwerk-task/src/evidence.rs:156` — используется на строке 121
  (зерно цепочки доказательств). Тест пакета M8-04 этот путь закрывает.
- `crates/stellwerk-task/src/approval.rs:183` — используется на строке 94
  (`empty_args_digest` от пустого буфера).

`crates/stellwerk-task/src/runner.rs:371` — тоже SHA-256 в hex, но это ИНЛАЙН в
вызывающем коде, а не функция. Он в пакете 07, чтобы ADR-0003 не лопнул.

## Что делать

1. Удалить обе приватные функции целиком, вместе с doc-комментами.
2. Добавить `use stellwerk_core::hexfmt::sha256_hex;` в оба файла.
3. Проверить, что в task осталось ноль определений:
   `grep -rn "fn sha256_hex" crates/stellwerk-task/src/` обязан вернуть пусто.

## Почему нельзя «просто удалить и не трогать вызовы»

Поведение обязано остаться **побайтово** прежним: `format!("{:x}", Sha256::digest(bytes))`.
Проверь, что у копий в task нет отличий от core-версии (сигнатура, формат, регистр).
Если отличие есть — это отдельная находка, верни BLOCKED с описанием, а не молча
унифицируй: две реализации могут означать два разных намерения.

## Приёмка

Главное доказательство — пересчёт, а не зелёная сьютка (F-100). Сравни с baseline
M8-01: task 394 total / 40 missed / 0 timeout. Удаление копий должно **не ухудшить**
числа: unviable может вырасти (мутанты удалённого кода исчезают), missed не должен.

Таймаут — утверждение о машине, не о коде (ADR-0014, F-101): проверь load < 40 перед
прогоном, артефакты в `~/gits/verify/`, TMPDIR в `/home`, вне репозитория.
