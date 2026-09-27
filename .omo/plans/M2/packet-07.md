# packet-07
plan_id: M2
goal: RED: адаптер песочницы — сборка окружения прогона из WorkspaceSpec и вызов `scripts/worker/sandbox.sh`.
files_allowed: [crates/stellwerk-task/src/sandbox.rs, crates/stellwerk-task/tests/sandbox.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-task --test sandbox
acceptance: RED: пакет собирается, тест компилируется, прогон падает (заглушка). Доказательство — вывод RED-wrapper.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [06]
data_class: public
lane: m2/sandbox-red

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

Песочница проекта уже есть — своя реализация ЗАПРЕЩЕНА. Используй
`scripts/worker/sandbox.sh`, который делает `git archive` в `/tmp` без `.git`.
Изоляция — не изобретание воркера, а уже принятое решение (`docs/PIVOT.md`).

1. `src/sandbox.rs`:
   - `pub struct Sandbox { root: PathBuf, keep: bool }`;
   - `pub fn prepare(base_commit: &str) -> Result<Sandbox, SandboxError>` — вызов
     `scripts/worker/sandbox.sh` через `std::process::Command`; путь к скрипту берётся
     из переменной окружения `STELLWERK_SANDBOX` (значение по умолчанию —
     `scripts/worker/sandbox.sh` в корне репозитория), чтобы тесты подставляли подделку;
   - `pub fn command_in(&self, program: &str, args: &[&str]) -> Command` — программа
     запускается ВНУТРИ песочницы, с `current_dir(root)`;
   - `pub fn contains_path(&self, p: &Path) -> bool` — защита от выхода за пределы.
2. `SandboxError` (thiserror): `Spawn { program, source }`, `Failed { status, stderr }`,
   `Outside(String)`.
3. Тесты в `tests/sandbox.rs`:
   - `contains_path` для пути внутри — true, для `..`-выхода наружу — false;
   - `prepare` с поддельным скриптом (создать во временном каталоге) → Ok, каталог создан;
   - `prepare` с падающим скриптом → Err;
   - `prepare` с несуществующим скриптом → Err.
   Команды в тестах — `sh -c` со скриптом, который сам создаёт/не создаёт каталог.
4. Никакого запуска Pi здесь — это packet-09+.

## Evidence

- RED-вывод `cargo test -p stellwerk-task --test sandbox`.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
