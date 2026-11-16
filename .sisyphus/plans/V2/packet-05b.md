---
id: V2/05b
task_id: V2-05b
milestone: V2
status: proposed
depends_on: [V2/05a, 05a, N1/01, N2/00h1, N2/00h2a, N2/00h3b, V1/04, V1/r01, V1/r02, V1/r04]
files_allowed: [crates/stellwerk-cli/src/lib.rs, crates/stellwerk-cli/tests/run.rs, crates/stellwerk-cli/examples/n1-smoke.toml]
max_files: 3
max_diff_lines: 200
acceptance: cargo test -p stellwerk-cli --test run
---

# V2/05b — F-05: run against a workspace copy and prove real Pi 1.0.2

## Цель
Цель: V2-05b F-05: run against a workspace copy and prove real Pi 1.0.2

## Шаг 1 — падающий тест
RED: новый тест с `<scratch>/ws/readme.txt` падает до копирования workspace; отсутствующий `ws` даёт `MissingWorkspace`; `run_task_with_real_pi` доказывает реальный Pi 1.0.2.

## Что делать
reuse `crates/stellwerk-cli/src/build_runner.rs::copy_workspace` at line 94 (already `pub fn`; do not widen or narrow its visibility); in `run_task_with`, after Task 2's `read_commit_sha` refusal, resolve `spec.workspace_ref` relative to the task file's parent directory (not the process cwd), require an existing directory, then copy it into `work` before building the worker. A missing or non-directory resolved `workspace_ref` is a named `MissingWorkspace` error, not an empty run. In `tests/run.rs`, add `run_task_with_real_pi` that calls `run_task_with(path, ModelConfig::with_token_path("http://127.0.0.1:<port>", HOST_MODEL_ID, token_path)?, &real_pi, &pi_extension())` directly with a local `TcpListener` model fixture; do not rely on `PI_BIN` env and do not use `run_cli_with`. Create the workspace fixture under a per-test scratch dir and set `workspace_ref` to its absolute path (absolute paths are used as-is; do not copy the repository root); set `examples/n1-smoke.toml` to `workspace_ref = "."` so it resolves to the task file parent `examples/`; the existing `run_writes_receipt_to_sqlite` and `both_entrypoints_share_one_authorization` tests stay green. Update the `workspace_ref = "ws"` fixtures at `tests/run.rs:172` and `:215` to create a real `<scratch>/ws` directory (or point them at an existing fixture) so `missing_engine_is_named_refusal` still reaches the fake Pi and prints `FAKE_PI_INVOKED`. The fake-Pi fixture must still pass. If Pi 1.0.2 is not installed, return BLOCKED with the exact missing path; do not skip. Must NOT copy `.git`, `target`, or `.omo`; use the existing copy rules. Must NOT silently fall back to an empty workspace.

## Приёмка
`cargo test -p stellwerk-cli --test run`; a new test creates `<scratch>/ws/readme.txt`, writes a scratch TOML with `workspace_ref` set to that absolute path, and asserts the agent sees the file; a missing `ws` returns `MissingWorkspace`; the existing `n1-smoke.toml` tests stay green. `PI_BIN=<real-pi-1.0.2> cargo test -p stellwerk-cli --test run run_task_with_real_pi` completes with the local fixture backend; evidence records the Pi version.

## QA
happy - the `ws` test; failure - delete `ws` and assert `MissingWorkspace`; real Pi - `cargo test -p stellwerk-cli --test run run_task_with_real_pi` via `run_task_with_real_pi`. Evidence `.omo/evidence/task-7-stellwerk-production-v2.md`.

## Запрещено
- do not widen or narrow its visibility)
- do not rely on `PI_BIN` env and do not use `run_cli_with`.
- do not copy the repository root)
- do not skip.
- Must NOT copy `.git`, `target`, or `.omo`
- Must NOT silently fall back to an empty workspace.

## Ссылки
`crates/stellwerk-cli/src/build_runner.rs:94-107` (`copy_workspace`); `crates/stellwerk-cli/src/lib.rs:140-180`; `crates/stellwerk-task/src/taskfile.rs:11-20` (`TaskFileError`), `:107` (`load`); `crates/stellwerk-cli/tests/run.rs:74-102` (`run_cli_with`), `:124-143` and `:294-306` (existing `n1-smoke.toml` tests), `:172`/`:215` (`workspace_ref = "ws"` fixtures); `crates/stellwerk-cli/examples/n1-smoke.toml`; `crates/stellwerk-cli/examples/pi-fixture.sh`.
