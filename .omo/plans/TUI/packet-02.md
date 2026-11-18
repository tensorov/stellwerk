# TUI-02 — состояние, чистый рендер, подкоманда `tui`

**label:** — · **mutation_gate:** no (view-слой; решение принимает `gateway::decide`)
**depends_on:** [TUI-01] · **ADR:** не требуется

## Цель

Дать TUI чистый шов рендера, который тестируется без TTY, и завести
подкоманду `stellwerk tui`. Решение владельца: **отдельная подкоманда**,
существующий `run` не трогается — его stdout-контракт
(`crates/stellwerk-cli/tests/run.rs:65,71,74` требуют поля `verdict`,
`receipt`, `store` при пипed stdio, `:96` требует их отсутствия при провале)
остаётся нетронутым, и база не ломается.

## production_call

`stellwerk-cli` → `cli.rs` (подкоманды) → `tui::run_tui`.

## files_allowed

- `crates/stellwerk-cli/src/tui.rs` (новый)
- `crates/stellwerk-cli/src/main.rs`
- `crates/stellwerk-task/src/cli.rs`

## Что делать

1. **`src/tui.rs`**: состояние `State` (путь к task-файлу, путь к агенту,
   фаза, вердикт, id receipt, риск, метка гейта) и **чистая функция рендера
   вида `fn render(state: &State) -> Vec<Line>`**, полностью отделённая от
   цикла событий и от терминала. Это обязательный шов: без него тест в
   `TestBackend` невозможен, потому что неотделимый рендер требует TTY.

2. **Экран обязан показывать**: task-файл, агента, действие `START`,
   а после старта — вердикт, id receipt, риск, состояние гейта.

3. **Настоящий Pi — только видимо отключённая опция**, с этой причиной
   дословно: требует model transport (N2-00) и pre-execution authorization
   инициированных моделью инструментов.

4. **`cli.rs`**: добавить подкоманду `tui`, принимающую путь к task-файлу.
   Имя бинаря `stellwerk` не меняется.

5. **Тест `crates/stellwerk-cli/tests/tui_render.rs`** (создать): строит
   `State` с заполненными полями и проверяет, что в буфере рендера есть
   вердикт, id receipt и строка «ждёт человека». Тест идёт в этом же пакете:
   закон 2 требует тест вместе с кодом.

## Запрещено трогать

`modules.toml`, `crates/stellwerk-core/**` (в т.ч. `contract.rs`),
`crates/stellwerk-gateway/**`, `crates/stellwerk-store/**`,
`crates/stellwerk-registry/**`, `justfile`, `.github/`,
`scripts/plan-lint.py`, `xtask/**`, `mutation-evidence/`.

**Запрещённые обещания** — ни одного из них не появляется: `cancel`,
`retry`, потоковая отрисовка токенов, очередь задач, авто-`Done`.
Гейт остаётся «ждёт человека».

**Существующий путь `run` не перестраивается** — это отдельный движок,
его правка вынесена в TUI-03 и ограничена там.

## Почему `mutation_gate: no`

Раскладка экрана. Решение о допуске принимает `gateway::decide`, а не вид.

## Приёмка

```sh
ssh -p 228 zeitoven.ru 'cd ~/gits/stellwerk && cargo test --workspace --no-fail-fast'; echo "WS_EXIT=$?"
```

`WS_EXIT=0`. База на момент планирования — 533 passed / 0 failed; число
не убывает. Если база отличается, зафиксировать фактическое значение
командой выше и опираться на него, а не на память.

```sh
ssh -p 228 zeitoven.ru 'cd ~/gits/stellwerk && cargo test -p stellwerk-cli --test tui_render'; echo "RENDER_EXIT=$?"
```

`RENDER_EXIT=0`.

```sh
ssh -p 228 zeitoven.ru 'cd ~/gits/stellwerk && grep -rn "name = \"stellwerk\"" crates/*/Cargo.toml | wc -l'
```

Ровно `1`.