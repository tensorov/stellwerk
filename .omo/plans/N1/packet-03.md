# N1-03 — контракт запуска: адаптер исполняет подготовленный sandbox-launch

**label:** contract-change · **mutation_gate:** yes (≥75%)
**depends_on:** [N1-02] · **ADR:** `docs/adr/0016-launch-contract.md` — обязателен

## Цель

Разрыв §2: `EngineAdapter::session` (`m5/mod.rs:176`) создаёт **голый**
`Command::new(program)` и спавтит сам. Через него невозможно провести ни bwrap,
ни `env_clear` — это блокер 3 из `REVIEW.md`: либо изоляция дыра, либо
production-вызов адаптера ложен.

**Решение 2:** адаптер исполняет **подготовленный** sandbox-launch. Окружение
и разрешённые mounts/network задаются **до** spawn. Второго production-пути
запуска мимо адаптера не создавать.

## production_call

- `m5::EngineAdapter::session` ← `orchestrator::SandboxWorker::exec`
- `m5::sandbox_bwrap::Sandbox::command` ← `orchestrator::SandboxWorker::exec`
  (**вызывается ДО** `session`, результат передаётся в него)

## files_allowed

- `docs/adr/0016-launch-contract.md`
- `crates/stellwerk-task/src/m5/mod.rs`
- `crates/stellwerk-task/src/orchestrator.rs`
- `crates/stellwerk-task/tests/engine_launch.rs`

**limit_waiver:** `files: 4` — ADR обязателен для contract-change, адаптер и
оркестратор меняются синхронно, тест обязателен по закону 2.

## ADR-0016 обязан зафиксировать

1. Почему адаптер получает подготовленный launch, а не собирает команду сам.
2. Что значит «второй production-путь» и почему он запрещён: если агент можно
   запустить в обход адаптера, bwrap и `env_clear` перестают быть гарантией,
   а остаются декорацией.
3. Что именно считается окружением: пустое окружение допускает **только**
   явно перечисленные переменные, и перечень задаётся до spawn.
4. Что считается окружением: пустое окружение допускает **только** явно
   перечисленные переменные, и перечень задаётся **до** spawn.
   Транспорт к модели в N1 **не строится** — он вынесен в N2-00
   (`index.md`): при `--unshare-net` наружу не выходит ничего, и глобальные
   credentials в окружение агента **не передаются**.

## Что делать

1. Изменить сигнатуру адаптера так, чтобы он принимал подготовленный launch
   (`Sandbox::command(...)` + явный набор переменных), а не собирал команду
   из `argv`. Это **узкая** правка: не переписывать `m5/*`, не менять
   `probe()`, не трогать `session_identity`, `integrator_profile`.
2. `orchestrator::SandboxWorker`:
   - вызывает `Sandbox::command(work, program, args, budget)`;
   - применяет `.env_clear()`, затем задаёт **только** разрешённые переменные;
   - передаёт подготовленный launch в `EngineAdapter::session`;
   - **не** содержит `Command::new` ни в одной ветке.
3. Никакого второго `Worker`-реализации, спавтящей напрямую.

## Почему `mutation_gate: yes`

Здесь решается «попадёт ли процесс под bwrap». Мутанты, которые убирают
`env_clear`, подменяют адаптер на прямой `Command::new` или разрешают полное
наследование окружения, обязаны быть пойманы.

## Запрещено трогать

`sandbox_bwrap.rs`, `secrets.rs`, `session_identity.rs`, `integrator_profile.rs`
— **не трогать**. Правка только в `m5/mod.rs` (сигнатура адаптера) и новом
`orchestrator.rs`. `crates/stellwerk-gateway/**`, `evidence.rs`, `runner.rs`,
`justfile`, `.github/`, `scripts/plan-lint.py`, `xtask/`.

## Приёмка

```sh
cargo build --workspace --all-targets; echo "BUILD_EXIT=$?"
cargo test -p stellwerk-task --test engine_launch --test orchestrator; echo "TEST_EXIT=$?"
```

**Оба 0.** Проверки:

```sh
# прямой Command::new в production-оркестраторе отсутствует
grep -rn "Command::new" crates/stellwerk-task/src/orchestrator.rs    # пусто

# env_clear применён
grep -rn "env_clear" crates/stellwerk-task/src/orchestrator.rs      # есть

# адаптер вызывается из production
grep -rn "\.session(" crates/stellwerk-task/src/ | grep -v tests/   # есть
```

Обязательный тест: регрессия, где адаптер вызывается **с подготовленным
launch**, обязана видеть bwrap-argv и пустое окружение. Если тест проверяет
только «процесс запустился» — пакет не выполнен: изоляция не доказана.

## Mutation

```sh
cargo mutants -p stellwerk-task --test-timeout 300 -- m5/mod.rs orchestrator
```

Порог 75% по `caught/(caught+missed)`; `unviable` в знаменатель не входит.
Не помещается во время — сузить scope; не помещается совсем — честно
`NOT_MEASURED`, а не «зелёный».