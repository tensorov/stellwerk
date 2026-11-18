---
id: V2/13
task_id: V2-13
milestone: V2
status: proposed
depends_on: [V2/05b, 05b]
files_allowed: [.omo/evidence/task-17-stellwerk-production-v2.md]
max_files: 3
max_diff_lines: 200
acceptance: cargo test -p stellwerk-cli --test audit_live -- --test-threads=1
---

# V2/13 — F-12/F-13: re-measure audit_live and the real-Pi budget

## Цель
Цель: V2-13 F-12/F-13: re-measure audit_live and the real-Pi budget

## Шаг 1 — падающий тест
Не TDD (evidence/measurement): трижды `cargo test -p stellwerk-cli --test audit_live -- --test-threads=1`, трижды `cargo test -p stellwerk-cli --test audit_live`, один раз `PI_BIN=<pi-1.0.2> cargo test -p stellwerk-cli --test build_runner`; результаты пишутся в evidence.

## Что делать
run `cargo test -p stellwerk-cli --test audit_live -- --test-threads=1` three times; run `cargo test -p stellwerk-cli --test audit_live` three times; run `PI_BIN=<pi-1.0.2> cargo test -p stellwerk-cli --test build_runner` once. Record wall-clock and pass/fail in `.omo/evidence/task-17-stellwerk-production-v2.md`. If a failure reproduces, create a FINDING and a separate budget packet; do not change `BUILD_WINDOW`/`BUILD_BUDGET` or `TEST_BUDGET_MS` speculatively. If Pi 1.0.2 is absent, return BLOCKED with the path. Must NOT use `#[ignore]` or skip.

## Приёмка
3x serial + 3x parallel `audit_live` all 7/7; one real-Pi `build_runner` run 17/17; evidence file contains the commands, timestamps, and results. A failure produces a FINDING, not a code change.

## QA
happy - the repeated commands; failure - a reproduced timeout is recorded and a FINDING is opened. Evidence `.omo/evidence/task-17-stellwerk-production-v2.md`.

## Запрещено
- do not change `BUILD_WINDOW`/`BUILD_BUDGET` or `TEST_BUDGET_MS` speculatively.
- Must NOT use `#[ignore]` or skip.

## Ссылки
`crates/stellwerk-cli/tests/audit_live.rs:17` (`TEST_BUDGET_MS = 50`); `crates/stellwerk-cli/tests/build_runner.rs:49-58` (60 s session); `crates/stellwerk-cli/src/build_runner.rs:584-585` (`BUILD_WINDOW`, `BUILD_BUDGET`); `.omo/evidence/final-wave-80ed4c1/f2-tests.log`.
