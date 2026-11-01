# N1-04 — миграция RunRequest, битый ShellSandbox уходит

**label:** contract-change · **mutation_gate:** no (миграция поля, решение 3)
**depends_on:** [N1-03, 02] · **ADR:** продолжает `docs/adr/0016-launch-contract.md`

## Цель

Разрыв §4: `runner.rs:18` импортирует `crate::sandbox::Sandbox`, а `RunRequest`
(`runner.rs:46`) несёт его **обязательным** полем. Это битый тип: его
`prepare()` гарантированно падает — он зовёт `sh scripts/worker/sandbox.sh`,
а скрипт содержит только функции без диспетчера argv, печатает пустоту,
`parse_root` возвращает `None`.

Это блокер 2 из `REVIEW.md`: требовалась правка `runner.rs`, который был и
запрещён, и вне `files_allowed`.

**Решение 3:** миграция `RunRequest` и точечная правка `runner.rs` разрешены.
Сломанный `ShellSandbox` **не остаётся обязательным фиктивным полем**. Старый
путь удаляется **только после** миграции всех потребителей.

## production_call

- `m5::sandbox_bwrap::Sandbox` ← `orchestrator::execute`
- `stellwerk_task::sandbox::Sandbox` — **production-вызовов не остаётся**

## files_allowed

- `crates/stellwerk-task/src/runner.rs`
- `crates/stellwerk-task/src/sandbox.rs`
- `crates/stellwerk-task/src/lib.rs`
- `crates/stellwerk-task/tests/sandbox.rs`

**limit_waiver:** `files: 4` — поле `RunRequest` и его тип объявлены в разных
файлах; удаление тестовой подмены обязательно по закону 2.

## Что делать

1. **Сначала миграция, потом удаление.** Порядок обязателен:
   - `RunRequest.sandbox` переводится на `m5::sandbox_bwrap::Sandbox`
     (тип из N1-03, тот же контракт запуска — ADR-0016);
   - только когда **все** потребители переведены, `src/sandbox.rs` удаляется
     и `pub mod sandbox` убирается из `lib.rs`.

2. Если перенос поля ломает `RunRequest` для существующих потребителей
   (`tests/vertical.rs` и др.) — править их **в этом же пакете** либо оставить
   переходным состоянием, объявленным явно. Молча оставить битый тип
   обязательным полем нельзя — это ровно то, что запрещает решение 3.

3. **Убрать тестовую подмену.** `tests/sandbox.rs:118` подставляет
   `STELLWERK_SANDBOX` фикстурой — именно она замаскировала дефект. Фикстура
   уходит; тест либо проверяет настоящий отказ, либо удаляется.

4. `lifecycle` и разбор потока **не переписывать без необходимости**
   (решение 3). Точечная правка — только тип поля и импорт.

## Почему `mutation_gate: no`

Пакет не пишет логику решений: он мигрирует тип поля и убирает неиспользуемый
путь. Измерять нечего.

## Запрещено трогать

`m5/sandbox_bwrap.rs`, `m5/secrets.rs`, `m5/session_identity.rs`,
`m5/integrator_profile.rs`, `m5/mod.rs` (сигнатура адаптера — из N1-03),
`crates/stellwerk-gateway/**`, `evidence.rs`, `justfile`, `.github/`,
`scripts/plan-lint.py`, `xtask/`.

`scripts/worker/sandbox.sh` **не удалять**: на него ссылаются
`scripts/worker/tests/run.sh` и историческая обвязка. N1 выводит shell-контур
из Rust-пути, но не разбирает его.

## Приёмка

```sh
cargo build --workspace --all-targets; echo "BUILD_EXIT=$?"
cargo test -p stellwerk-task --test sandbox --test vertical; echo "TEST_EXIT=$?"
```

**Оба 0.** Workspace обязан собираться целиком: пакет удаляет тип, который
импортируется в `runner.rs`, — падение компиляции здесь означает, что
миграция не доведена.

Проверки:

```sh
# битый тип исчез из production
grep -rn "crate::sandbox\|sandbox::Sandbox" crates/stellwerk-task/src/ || echo "ссылок нет"

# тестовая подмена ушла
grep -rn "STELLWERK_SANDBOX" crates/stellwerk-task/tests/ || echo "фикстуры нет"
```

Обе команды обязаны печатать `ссылок нет` / `фикстуры нет`.