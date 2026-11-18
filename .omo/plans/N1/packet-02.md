# N1-02 — ExecutionSpec: как назвать программу агента, и отказ без неё

**label:** contract-change · **mutation_gate:** no (контракт-типы, решение 1)
**depends_on:** [N1-01] · **ADR:** `docs/adr/0015-execution-spec-not-task-spec.md`

## Цель

Разрыв п.1: Rust не может указать, что запускать. **Решение 1:** `TaskSpec`
не расширяется — вводится отдельная явная `ExecutionSpec` на стороне
orchestration.

`TaskSpec` не имеет `Default`, и добавление полей ломает 5 тестовых хелперов в
`task` и `store`, которых нет в `files_allowed` (это блокер 1 из `REVIEW.md`).
Отдельная структура обходит проблему, не трогая существующий контракт задачи.

## production_call

- `orchestrator::ExecutionSpec` ← `stellwerk_cli::run::execute`
- отказ «нет конфигурации» ← тот же `execute`, **до** `run_once`

## files_allowed

- `docs/adr/0015-execution-spec-not-task-spec.md`
- `crates/stellwerk-task/src/execution.rs`
- `crates/stellwerk-task/src/lib.rs`
- `crates/stellwerk-task/tests/execution.rs`

**limit_waiver:** `files: 4` — ADR обязателен для `contract-change`; тип,
его регистрация в `lib.rs` и тест меняются синхронно.

## Что делать

1. **ADR-0015**: почему не `TaskSpec`. Ключевые доводы: `TaskSpec` — это
   *что сделать*, а программа агента — это *чем выполнить*; смешивать их
   значит заставить `store` знать о запуске. Плюс: `TaskSpec` без `Default`,
   расширение ломает чужие конструкторы.

2. `ExecutionSpec` — отдельный тип на стороне task:
   - `program: String` — путь или имя исполняемого файла;
   - `engine: String` — идентификатор движка;
   - поля **без `Default`** и **без `#[serde(default)]`**.

3. **Реестр разрешённых engine-профилей** в коде. CLI выбирает профиль из
   реестра; выбранный профиль **сохраняется в Run** вместе с `ExecutionSpec`,
   чтобы в evidence было видно, чем именно запускали.

4. **Нет конфигурации → отказ до запуска** (решение 1, закон 6 AGENTS.md):
   - неизвестный профиль → отказ с именем профиля;
   - отсутствует `ExecutionSpec` → отказ;
   - `program` пуст → отказ.
   **Скрытый default запрещён.** Отказ возвращается как тип, а не как
   «запустим что есть».

5. Отказ происходит **до** `run_once` и **до** создания sandbox: ни одного
   процесса не стартует.

## Почему `mutation_gate: no`

Контракт-типы и разбор конфигурации. Решение о допуске принимает
`gateway::decide`, а не этот тип; измерять нечего.

## Запрещено трогать

`crates/stellwerk-core/src/contract.rs` — **`TaskSpec` не трогаем вообще**
(решение 1). `taskfile.rs` не трогаем — новых полей в TOML нет.
`crates/stellwerk-task/src/m5/**`, `runner.rs`, `evidence.rs`,
`crates/stellwerk-gateway/**`, `justfile`, `.github/`,
`scripts/plan-lint.py`, `xtask/`.

## Приёмка

```sh
cargo build --workspace --all-targets; echo "BUILD_EXIT=$?"
cargo test -p stellwerk-task --test execution; echo "TEST_EXIT=$?"
```

**Оба 0.** Обязательные тесты:

- `TaskSpec`-литералы в `lifecycle.rs`, `inbox.rs`, `events.rs`,
  `status_transition.rs`, `task_crud.rs` **остались компилируемыми** —
  проверить полной сборкой workspace, а не одним таргетом;
- неизвестный engine → отказ, **процесс не стартовал**;
- отсутствие `ExecutionSpec` → отказ, не «запустить по умолчанию».

Проверка, что контракт задачи не тронут:

```sh
git diff --stat crates/stellwerk-core/src/contract.rs crates/stellwerk-task/src/taskfile.rs
```

Ожидается пусто.