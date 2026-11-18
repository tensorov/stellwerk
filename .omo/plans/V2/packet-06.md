---
id: V2/06
task_id: V2-06
milestone: V2
status: proposed
depends_on: [V2/05b, 05b, N2/00h1, N2/00h2a, N2/00h3b, V1/r01, V1/r02, V1/r04]
files_allowed: [crates/stellwerk-task/src/taskfile.rs, crates/stellwerk-cli/src/lib.rs, crates/stellwerk-task/tests/taskfile.rs]
max_files: 3
max_diff_lines: 200
acceptance: cargo test -p stellwerk-task --test taskfile --test execution
overlap_waiver: C9 — V1/r01, V1/r02, V1/r04, N2/00h1, N2/00h2a, N2/00h3b — общие файлы с более ранними milestone, строго последовательное исполнение в одном worktree
---

# V2/06 — F-07: route execution admission through execution::resolve

## Цель
Цель: V2-06 F-07: route execution admission through execution::resolve

## Шаг 1 — падающий тест
RED: тесты в `tests/taskfile.rs` падают, пока `resolve(program)` не делегирует в `execution::resolve`; пустой program обязан дать `TaskFileError::EmptyProgram`.

## Что делать
rename the current `TaskFile::resolve` to a private `resolve_profile(&self) -> Result<&'static EngineProfile, TaskFileError>` (only `validate` calls it at `taskfile.rs:135`); add a new `pub fn resolve(&self, program: &str) -> Result<ResolvedExecution, TaskFileError>` that binds `let profile = self.resolve_profile()?;`, builds `ExecutionSpec { program: program.to_owned(), engine: profile.engine().to_owned() }`, and calls `execution::resolve(Some(&spec), &REGISTRY)`; map `ExecutionRefusal::EmptyProgram` to `TaskFileError::EmptyProgram` and `ExecutionRefusal::UnknownProfile { engine, known }` to `TaskFileError::UnknownProfile { profile: engine, known }`. There is no `Absent` arm: `Some(&spec)` is always present, so add only `EmptyProgram` to `TaskFileError` and do not create an unreachable `AbsentExecutionSpec`. Keep `validate` on the private `resolve_profile()` with the old engine-only lookup. In `crates/stellwerk-cli/src/lib.rs::run_task_with`, bind `TaskFile` before `into_spec`; compute `resolved = task_file.resolve(pi.to_str().ok_or("MissingEngine: non-UTF-8 program path")?).map_err(|e| e.to_string())?`; use `resolved.program()` for the worker. Add the tests in `tests/taskfile.rs` (not a `#[cfg(test)]` module) so the packet stays at 3 files; a blank program yields `TaskFileError::EmptyProgram`, a valid program yields `ResolvedExecution` with the same program. Must NOT keep two admission paths. Must NOT default a missing profile to `pi`. Must NOT add a second registry.

## Приёмка
`cargo test -p stellwerk-task --test taskfile --test execution`; `cargo test -p stellwerk-cli --test run`; `rg -n "execution::resolve|task_file\\.resolve" crates/stellwerk-task/src/taskfile.rs crates/stellwerk-cli/src/lib.rs` shows both the delegation and the live call; a missing profile still returns `MissingProfile`, an unknown profile returns `UnknownProfile` with `known == ["pi"]`, and a blank program returns `EmptyProgram`.

## QA
happy - the taskfile/execution tests and `cargo test -p stellwerk-cli --test run`; failure - `profile = "fixture-engine"` returns `UnknownProfile`; `profile` absent returns `MissingProfile`; `program = ""` returns `EmptyProgram`. Evidence `.omo/evidence/task-8-stellwerk-production-v2.md`.

## Запрещено
- There is no `Absent` arm: `Some(&spec)` is always present, so add only `EmptyProgram` to `TaskFileError` and do not create an unreachable `AbsentExecutionSpec`.
- Must NOT keep two admission paths.
- Must NOT default a missing profile to `pi`.
- Must NOT add a second registry.

## Ссылки
`crates/stellwerk-task/src/execution.rs:120-215` (`EngineRegistry`, `ResolvedExecution`, `resolve`); `crates/stellwerk-task/src/taskfile.rs:9-30` (`TaskFileError`); `crates/stellwerk-task/src/taskfile.rs:119-137` (`resolve_profile`, `resolve`, `validate`); `crates/stellwerk-cli/src/lib.rs:111-170` (`run_task`); `crates/stellwerk-task/tests/taskfile.rs`; `crates/stellwerk-task/tests/execution.rs`; `docs/adr/0021-single-execution-profile.md`.
