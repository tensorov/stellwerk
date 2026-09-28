# packet-04
plan_id: M4
goal: GREEN — `watch <id>` печатает события задачи, а не остаётся заглушкой.
files_allowed: [crates/stellwerk-task/src/cli.rs, crates/stellwerk-task/tests/cli_watch.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-task --test cli_watch
acceptance: `cargo test -p stellwerk-task --test cli_watch` → 0 failed. `apply` и `get` не сломаны: `cargo test -p stellwerk-task --lib cli` → 0 failed.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [M2-01, M2-03, M2-04, M2-12]
data_class: public
lane: m4/cli

## Контекст

- В M2-04 `watch` — честная заглушка с кодом 1 и текстом
  `not implemented: watch (packet-12)`. Этот пакет заменяет **только `watch`**;
  `cancel` и `logs` остаются заглушками и указывают на свои пакеты.
- `watch` читает `EventStore::list_by_task`. Трейт уже есть в
  `stellwerk-store/src/traits.rs`; реализация — SqliteStore из M2-12.
- **Зависит от M2-12:** без записи событий команде нечего показывать. Раздавать
  раньше нельзя — это неисполнимость по существу, а не задержка по времени.

## Что сделать

1. `watch <id>`: `EventStore::list_by_task(&id)`. Пустой список — код 0 и
   `no events for <id>` на stdout, **не** код 1: отсутствие событий для
   существующей задачи — это факт, а не ошибка.
2. Печать по одному событию на строку в порядке, который отдаёт трейт. Формат
   строки: `<timestamp> <kind> <краткое описание>`. Время и kind берутся из
   `Event`, ничего не выдумывается.
3. Отсутствующая задача — код 1, как у `get`. **Существующая задача без событий —
   код 0.** Различать эти два случая обязательно, иначе `watch` на только что
   созданной задаче выглядит как ошибка.
4. Убрать заглушку `watch` из `run` и обновить её текст, если он где-то
   перечислен как «не реализовано». `cancel` и `logs` не трогать.

## Evidence

- `cargo test -p stellwerk-task --test cli_watch` → 0 failed.
- `cargo test -p stellwerk-task --lib cli` → 0 failed, регрессии нет.
- clippy `-D warnings` и `fmt --check` чисто.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
