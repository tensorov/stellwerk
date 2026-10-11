# packet-04 — core получает единственное определение sha256_hex

plan_id: M8
status: предложен
goal: GREEN — в stellwerk-core есть единственное определение sha256_hex, покрытое тестом на векторах стандарта, и ни одной копии в крейтах-потребителях на этом шаге не появилось
files_allowed: [crates/stellwerk-core/Cargo.toml, crates/stellwerk-core/src/hexfmt.rs, crates/stellwerk-core/src/lib.rs]
max_diff_lines: 200
test: cargo mutants --package stellwerk-core --jobs 18 --timeout 300
acceptance: cargo mutants --package stellwerk-core показывает missed=0 для hexfmt; cargo clippy -p stellwerk-core --all-targets чист; cargo test -p stellwerk-core зелёный
forbidden: [трогать crates/stellwerk-core/src/contract.rs, трогать копии в task и gateway (это пакеты 05 и 06), переиспользовать векторы без сверки с sha256sum, ослабить проверку ради зелёного цвета]
depends_on: [03]
mutation_threshold: 80
data_class: private
lane: runtime

## Контекст

Инвентаризация M8-03 (`.stellwerk/DUPLICATES.md`, семейство A2) нашла шесть мест,
где продакшен печатает SHA-256 в hex:

- `crates/stellwerk-gateway/src/session.rs:385`
- `crates/stellwerk-gateway/src/shaping.rs:133` (побайтовый hex, другая реализация)
- `crates/stellwerk-task/src/approval.rs:185`
- `crates/stellwerk-task/src/evidence.rs:157`
- `crates/stellwerk-task/src/runner.rs:371` (инлайн в вызывающем коде)

Пакет создаёт единственное определение в core. Переключение потребителей — пакеты
05 и 06, здесь они НЕ трогаются: ADR-0003 даёт 3 файла на пакет, а создание модуля
в core само занимает 3 (манифест, новый файл, объявление в lib.rs).

## Почему именно так разбито

`stellwerk-core` сегодня зависит только от `serde`. Для хеша нужен `sha2`, которого
в core нет, а в `[workspace.dependencies]` корневого `Cargo.toml` он есть
(`sha2 = "0.10.9"`). Подключать через `workspace = true` — решение владельца от
2026-10-11, и оно НЕ является `contract-change`: защищён только корневой Cargo.toml,
а `crates/stellwerk-core/Cargo.toml` под защиту не попадает.

Добавление зависимости и создание функции нельзя объединить с переключением
первого потребителя: это 4-5 файлов, пакет превысит ADR-0003.

## Требования к функции

- Сигнатура `pub fn sha256_hex(bytes: &[u8]) -> String`, поведение побайтово то же,
  что у существующих копий: `format!("{:x}", Sha256::digest(bytes))`.
- `shaping.rs:133` НЕ копировать: там побайтовый hex, у него своя покрытая
  семантика (учёт длины и регистра), это отдельная функция с другими тестами.
- Тест — инлайновый `#[cfg(test)] mod` ВНУТРИ `hexfmt.rs`: отдельный файл в
  `crates/stellwerk-core/tests/` дал бы 4-й файл и развалил пакет.
- **Оракул — константы стандарта, записанные литералами, а не перереализация.**
  Если оракул сам считает SHA-256, он не проверяет ничего: пакет поймал бы свой же
  баг. Векторы сверяются с `sha256sum` ДО зашивки, результат сверки приводится в отчёте.

## Почему это может упасть и что делать

`contract.rs` запрещён к правке без contract-change. Он содержит эквивалентные
мутанты (по baseline M8-01 — всего 3 missed в core). Их нельзя убить, и они не
относятся к этому пакету: `hexfmt.rs` в мутации не участвует.

Если прогон покажет missed, отличный от трёх известных эквивалентных из
`contract.rs` — значит тест недостаточен, и его надо дополнить векторами. Ослаблять
проверку нельзя. Если таймауты — проверить load (ADR-0014, F-101): таймаут на
загруженной машине это утверждение о машине, а не о коде.

## Шаги

1. Проверить x99: `git status --porcelain` пуст, `load < 40`, нет живых `rustc`.
2. Сверить векторы через `sha256sum` на x99, вывод приложить к отчёту.
3. `crates/stellwerk-core/Cargo.toml`: `sha2 = { workspace = true }`.
4. `crates/stellwerk-core/src/hexfmt.rs`: функция + инлайновый тест.
5. `crates/stellwerk-core/src/lib.rs`: `pub mod hexfmt;`.
6. `cargo test -p stellwerk-core`, `cargo clippy -p stellwerk-core --all-targets`.
7. `cargo mutants --package stellwerk-core --jobs 18 --timeout 300` на простое
   машине, артефакты в `~/gits/verify/`, TMPDIR в `/home`, вне репозитория.
