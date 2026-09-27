# packet-08
plan_id: M2
goal: Реализовать адаптер песочницы поверх `scripts/worker/sandbox.sh`.
files_allowed: [crates/stellwerk-task/src/sandbox.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-task --test sandbox
acceptance: `cargo test -p stellwerk-task --test sandbox` → все тесты зелёные; ни один выход за пределы песочницы не проходит.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [07]
data_class: public
lane: m2/sandbox

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

1. `prepare`: `Command::new("sh").arg(script).arg(base_commit)` с перехватом
   stdout/stderr. Непустой exit → `SandboxError::Failed`. Путь создаётся скриптом —
   парси его stdout, а не угадывай.
2. НИКОГДА не вызывай `rm`: очистку делает `sandbox.sh` (у него есть `SANDBOX_KEEP`).
   В крейте не должно быть ни одного вызова удаления — это проверяется ревью.
3. `command_in`: `Command::new(program).current_dir(&self.root).args(args)`.
4. `contains_path`: канонизировать и проверить, что результат — префикс `root`.
   `canonicalize` на несуществующем пути падает — обработай: несуществующий путь
   считай «внутри», только если он не выходит через `..` (тест на `..`-выход).
5. Убери `NotImplemented` из ошибок.

## Evidence

- `cargo test -p stellwerk-task --test sandbox` → passed.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
