# packet-02
plan_id: M2
goal: Реализовать загрузку TaskSpec из TOML: `from_toml_str`, `load`, `into_spec`.
files_allowed: [crates/stellwerk-task/src/taskfile.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-task --test taskfile
acceptance: `cargo test -p stellwerk-task --test taskfile` → 5 passed; 0 failed. `cargo test -p stellwerk-core` и `-p stellwerk-store` не меняются.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [01]
data_class: public
lane: m2/taskfile

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

- `TaskFile` — зеркало входного TOML, а НЕ копия `TaskSpec`:
  входной файл может быть плоским, контракт — вложенным.

## Что сделать

1. Опиши `TaskFile` с `#[serde(deny_unknown_fields)]`: неизвестный ключ во входном файле —
   ошибка, иначе опечатка в имени лимита тихо отключит ограничение.
2. `TaskFileError` (thiserror): `Toml(#[from] toml::de::Error)`, `Io(#[from] std::io::Error)`,
   `MissingField(&'static str)`, `Validation(String)`.
3. `from_toml_str`: `toml::from_str`; отсутствие обязательного поля → `MissingField`.
4. Валидация, которая не выражается типами: `max_files > 0`, `max_diff_lines > 0`,
   `timeout_seconds > 0`, `id` и `goal` непустые. Нарушение → `Validation`.
5. `load(&Path)`: `std::fs::read_to_string` + `from_toml_str`.
6. `into_spec(self) -> Result<TaskSpec, TaskFileError>`: собери `TaskSpec` из
   `stellwerk_core::contract` — структуру НЕ дублируй, она уже в контракте.
7. Убери `NotImplemented` из ошибки, если заводила.

## Evidence

- `cargo test -p stellwerk-task --test taskfile` → 5 passed.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
