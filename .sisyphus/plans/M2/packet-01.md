# packet-01
plan_id: M2
goal: Создать крейт `stellwerk-task`: Cargo.toml, lib/main, и падающий тест загрузки TaskSpec из TOML. contract-change: добавляется член workspace.
files_allowed: [Cargo.toml, crates/stellwerk-task/Cargo.toml, crates/stellwerk-task/src/lib.rs, crates/stellwerk-task/src/main.rs, crates/stellwerk-task/tests/taskfile.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-task --test taskfile
acceptance: RED: `bash -c 'cargo build -p stellwerk-task && cargo test -p stellwerk-task --test taskfile --no-run && ! cargo test -p stellwerk-task --test taskfile'` → exit 0: крейт собирается, тест компилируется, прогон падает на assert, а НЕ на ошибке компиляции.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [—]
data_class: public
lane: m2/skeleton

## Контекст

- **Новый крейт `stellwerk-task`.** Ничего из `stellwerk-core` и `stellwerk-store`
  не редактируется: крейт только вызывает их публичные API.
- Крейт зависит от `stellwerk-core` (контракт) и `stellwerk-store` (трейты) по path.
  Зависимости — только из `[workspace.dependencies]`. `cargo add` запрещён.
- Команда kubectl-shaped: `stellwerk task apply|get|watch|cancel|logs`.
  Один глагол — одно действие; комбо-команд нет.
- Задача описывается TOML-файлом (`toml` уже в workspace) — YAML-крейт не нужен.
- Статусы ведутся ЧЕРЕЗ предикаты ядра (`TaskStatus::can_transition_to`). Не заводи
  собственный свод переходов: единственный источник истины — контракт.
- Уже закрытые пакеты M1 (packet-02…12) — их предикаты и трейты считай готовыми.

## Что сделать

**Помечен contract-change:** правка корневого `Cargo.toml` (добавление члена `members`)
разрешена только в этом пакете и только этой строкой.

1. Корневой `Cargo.toml`: добавить `crates/stellwerk-task` в `members`. Больше ничего.
2. `crates/stellwerk-task/Cargo.toml`: зависимости `stellwerk-core` и `stellwerk-store`
   по path, плюс из workspace — `serde`, `serde_json`, `toml`, `thiserror`, `clap`, `tokio`.
   Новых крейтов не добавлять.
3. `src/lib.rs`: `pub mod taskfile;` и `pub mod cli;` (заглушки-модули, пустые).
4. `src/main.rs`: `fn main() {}` — вызов CLI в packet-04.
5. RED-тест в `tests/taskfile.rs` (фильтр `taskfile`):
   - FIXTURE TOML с `[task] id/goal/workspace_ref/model_ref/packet`,
     `[task.limits]` (max_files, max_diff_lines, timeout_seconds, max_retries,
     context_budget_tokens, data_class) и `evidence_policy`;
   - `TaskFile::from_toml_str(FIXTURE)` → Ok, все поля разобраны, `data_class == Internal`;
   - `TaskFile::from_toml_str` с неизвестным ключом → Err;
   - `TaskFile::from_toml_str` с `max_files = -1` → Err (u32, отрицательное не парсится);
   - `TaskFile::into_spec()` конвертирует в `TaskSpec` из контракта, id и goal совпадают.
   Все пять падают: тело функций — заглушка, возвращающая Err.
6. **Не** реализуй загрузку: это packet-02.

## Evidence

- RED-вывод `cargo test -p stellwerk-task --test taskfile` (падает, причина — заглушка).
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
