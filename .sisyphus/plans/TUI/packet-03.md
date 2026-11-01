# TUI-03 — START, запуск существующего пути, риск из реестра

**label:** — · **mutation_gate:** no (обвязка запуска; решение — `gateway::decide`)
**depends_on:** [TUI-02, 02] · **ADR:** не требуется
overlap_waiver: C9 — N1 и N2 делят crates/stellwerk-cli/src/main.rs, последовательное исполнение

## Цель

Нажать `START` должно запускать **существующий** путь исполнения, а риск на
экране обязан приходить из реестра, а не из строки, собранной на месте.

Ключевой факт, установленный ревью: вывод `main.rs:234` печатает
`verdict={} receipt={} store={} gate={}` — **поля риска там нет**. Риск
живёт только внутри, в `CLI_AGENT_TOOLS` (`main.rs:41`) и
`ToolRisk::Classified(RiskClass::R0)` (`main.rs:164`), и наружу не выводится.
Поэтому «показать зарегистрированный риск» невозможно без явного решения,
и оно такое: риск **добавляется в вывод одной строкой**, и значение берётся
из реестра.

## production_call

`stellwerk tui <task.toml>` → существующий путь запуска
(`main.rs:145` `SandboxWorker::new(FixtureEngine::new("/bin/true"), …)`,
`:153` `.with_registry(StaticRegistry(CLI_AGENT_TOOLS))`, далее `CallAttempt`,
ledger, `evidence_status` `:233`).

## files_allowed

- `crates/stellwerk-cli/src/main.rs`
- `crates/stellwerk-cli/src/tui.rs`
- `crates/stellwerk-cli/tests/tui_start.rs`

## Что делать

1. **Действие `START`** в цикле событий вызывает существующий путь запуска.
   Отдельный движок исполнения **запрещён** — переиспользовать тот же.

2. **Риск на экране берётся из `CLI_AGENT_TOOLS`.** Допустимо ровно одно
   расширение вывода: поле `risk=` рядом с `verdict`/`receipt`/`store`/`gate`
   в `main.rs:234`, значение — результат lookup по тому же реестру, который
   уже используется для решения. Любой иной источник значения запрещён:
   строка риска, не связанная с реестром, — это ровно дефект ADR-0017,
   закрытый в N1.

3. **Тест `tests/tui_start.rs`**: проверяет, что риск, показанный на экране,
   совпадает с классом из реестра, и что при отказе реестра и при
   `Ok(Unclassified)` на экране нет признака исполнения. Совпадение проверяется
   на **разных** значениях — тест, который проходит и при совпадении, и при
   расхождении, не считается.

## Запрещено трогать

Подкоманда `run` и её stdout-контракт. `modules.toml`,
`crates/stellwerk-core/**` (в т.ч. `contract.rs`), `crates/stellwerk-gateway/**`,
`crates/stellwerk-store/**`, `crates/stellwerk-registry/**`, `justfile`,
`.github/`, `scripts/plan-lint.py`, `xtask/**`, `mutation-evidence/`.

Запрещены `cancel`, `retry`, поток токенов, очередь задач, авто-`Done`.
Гейт остаётся «ждёт человека».

## Почему `mutation_gate: no`

Обвязка запуска и вывод. Решение о допуске принимает `gateway::decide`.

## Приёмка

```sh
ssh 192.168.1.11 'cd ~/gits/stellwerk && cargo test --workspace --no-fail-fast'; echo "WS_EXIT=$?"
```

`WS_EXIT=0`, база не убывает.

```sh
ssh 192.168.1.11 'cd ~/gits/stellwerk && cargo test -p stellwerk-cli --test tui_start'; echo "START_EXIT=$?"
```

`START_EXIT=0`.

```sh
ssh 192.168.1.11 'cd ~/gits/stellwerk && cargo test -p stellwerk-cli --test run'; echo "RUN_EXIT=$?"
```

`RUN_EXIT=0` — контракт существующей подкоманды `run` не сломан.