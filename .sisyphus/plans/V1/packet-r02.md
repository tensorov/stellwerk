---
id: r02
milestone: V1
status: GREEN
depends_on: [02, 01]
files_allowed: [crates/stellwerk-task/src/taskfile.rs, crates/stellwerk-task/src/execution.rs, crates/stellwerk-task/tests/taskfile.rs]
overlap_waiver: C9 — N1/02 — `execution.rs`, последовательное исполнение
acceptance: cargo test -p stellwerk-task --test taskfile
---

# r02 — `task.toml` несёт только `profile = "pi"`

## Цель
`task.toml` требует явное поле `profile` и принимает только `pi`, потому что отсутствие поля, прочитанное как `pi`, делает отказ неотличимым от согласия.

## Обоснование (измерено)
- `crates/stellwerk-task/src/taskfile.rs:32` — здесь объявлен `TaskFileSpec`, и поля `profile` в нём нет.
- `crates/stellwerk-task/src/taskfile.rs:118` — разбор `task.toml` заканчивается здесь, куда и добавляется проверка профиля.
- `crates/stellwerk-task/src/execution.rs:188` — `resolve` — точка, где профиль сверяется с разрешённым списком.

## Шаг 1 — падающий тест
`crates/stellwerk-task/tests/taskfile.rs`, тест `profile_must_be_explicit_pi`: утверждает, что `profile = "pi"` принимается, а `profile = "opencode"` и отсутствие поля дают два разных названных отказа; до правки поля нет, поэтому `deny_unknown_fields` отвергает `profile`, а отсутствие поля проходит молча — тест падает на обоих утверждениях.

## Приёмка
cargo test -p stellwerk-task --test taskfile
Ожидается PASS.

## Запрещено
- нельзя добавлять дефолт «нет поля — значит pi».
- запрещено принимать любое значение `profile`, кроме `pi`.
- нельзя сливать отсутствие профиля и неверный профиль в один отказ.
