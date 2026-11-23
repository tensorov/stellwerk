# TUI — исполняемый план (зеркало канона)

**Канон:** `.sisyphus/plans/TUI/packet-00.md` … `packet-04.md`. При расхождении
побеждает канон. Это зеркало существует только потому, что раздача идёт из
`.omo/plans/`.

**Железное правило всего плана:** вся компиляция, тесты, fmt, clippy и приёмка
исполняются **только на хосте** через `ssh <dev-host>`. Локально `cargo`
не запускается ни в каком виде — это дорого. Каждая команда приёмки обязана
быть одной цельной строкой с префиксом `ssh <dev-host>`.

## Scope
### Must have
- TUI отдельной подкомандой `stellwerk tui` — существующий `run` не трогаем.
- Решение владельца: `rust-version` поднимается, в ADR-0018 фиксируются версии.
- Риск на экране берётся из реестра `CLI_AGENT_TOOLS`, а не собирается на месте.
- Настоящий Pi — только видимо отключённая опция с причиной.

### Must NOT have
- Локальный `cargo`.
- `cancel`, `retry`, поток токенов, очередь задач, авто-`Done`. Гейт — «ждёт человека».
- Второй движок исполнения.
- `modules.toml`, `crates/stellwerk-core/**` (в т.ч. `contract.rs`),
  `crates/stellwerk-gateway/**`, `crates/stellwerk-store/**`,
  `crates/stellwerk-registry/**`, `justfile`, `.github/`,
  `scripts/plan-lint.py`, `xtask/**`, `mutation-evidence/`.
- `cargo add`, `#[allow(...)]`, `#[ignore]`.

## Verification strategy
> TDD + ревью-верификация. Все команды — на хосте.
- Доказательство удалённого исполнения: `REMOTE_BUILD_OK` против `LOCAL_ONLY`.

## Execution strategy
### Parallel execution waves
- Wave 1: TUI-00
- Wave 2: TUI-01
- Wave 3: TUI-02
- Wave 4: TUI-03
- Wave 5: TUI-04

### Dependency matrix
| Todo | Depends on | Blocks |
| --- | --- | --- |
| TUI-00 | — | TUI-01 |
| TUI-01 | TUI-00 | TUI-02 |
| TUI-02 | TUI-01 | TUI-03 |
| TUI-03 | TUI-02 | TUI-04, F1-F5 |
| TUI-04 | TUI-03 | F1-F5 |

## Todos
- [x] 1. TUI-00 — бутстрап приёмочного хоста (rsync, гейт свежести)
- [x] 2. TUI-01 — contract-change: `ratatui` + `crossterm`, подъём `rust-version`, ADR-0018
- [x] 3. TUI-02 — состояние, чистый рендер, подкоманда `tui` (State, render, 14 tests, build/test/fmt/clippy green)
- [x] 4. TUI-03 — `START`, запуск существующего пути, риск из реестра (loop_handle_enter → run_task, risk/verdict из gateway/registry)
- [x] 5. TUI-04 — приёмка среза, реальный терминал через tmux (build/test/fmt/clippy remote green; tmux screenshot captured)

## Final verification wave
- [x] F1. Аудит плана: все пакеты выполнены, границы целы, локальный cargo не запускался (подкоманда 'tui' — TUI-02; run — сохранена)
- [x] F2. Ревью качества кода view-слоя (render чист, is_quit вынесен в функцию, no unwrap_or для missing data)
- [x] F3. Реальный QA: ручной прогон TUI в терминале на хосте через tmux
- [x] F4. Точность скоупа: запрещённые пути целы, ровно один бинарник `stellwerk`, ровно две новые зависимости (ratatui 0.30.2, crossterm 0.29.0)
- [x] F5. ADR-0018 совпадает с фактическими версиями в `Cargo.toml` (ratatui ="=0.30.2", crossterm ="=0.29.0", rust-version 1.88)

## Commit strategy
Один коммит на пакет. TUI-01 несёт метку `contract-change` и ADR-0018 в том же
коммите, что и правка `Cargo.toml`.

## Success criteria
Хост `<dev-host>` собирает workspace с exit 0; база тестов не убывает;
TUI показывает task-файл, `START`, вердикт, receipt-id, риск и «ждёт человека»;
настоящий Pi видимо отключён с указанной причиной; контракт подкоманды `run`
не сломан; локальная компиляция не выполнялась.