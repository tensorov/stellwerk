---
id: V2/10
task_id: V2-10
milestone: V2
status: proposed
depends_on: []
files_allowed: [.sisyphus/plans/V1/packet-01.md]
max_files: 3
max_diff_lines: 200
acceptance: cargo test -p stellwerk-gateway --test session
---

# V2/10 — F-11: align V1/packet-01 with the shipped Unpark+park

## Цель
Цель: V2-10 F-11: align V1/packet-01 with the shipped Unpark+park

## Шаг 1 — падающий тест
Не TDD (docs packet): проверка — `! rg -n "Waker::noop" .sisyphus/plans/V1/packet-01.md` и `cargo test -p stellwerk-gateway --test session`.

## Что делать
replace the `Waker::noop()` prescription in `.sisyphus/plans/V1/packet-01.md` with `Unpark(thread::current()) + park()`; update the goal/step text so it describes the shipped implementation and the existing test. Keep `status: GREEN`. Must NOT change `crates/stellwerk-gateway/src/session.rs` or weaken `tests/session.rs`.

## Приёмка
`! rg -n "Waker::noop" .sisyphus/plans/V1/packet-01.md` succeeds; `rg -n "Unpark|park" .sisyphus/plans/V1/packet-01.md` matches; `cargo test -p stellwerk-gateway --test session` passes.

## QA
happy - the `rg` checks and `cargo test -p stellwerk-gateway --test session`; failure - the old `Waker::noop` text must fail the `rg` check. Evidence `.omo/evidence/task-14-stellwerk-production-v2.md`.

## Запрещено
- Must NOT change `crates/stellwerk-gateway/src/session.rs` or weaken `tests/session.rs`.

## Ссылки
`.sisyphus/plans/V1/packet-01.md:15`; `crates/stellwerk-gateway/src/session.rs:618-630`; `crates/stellwerk-gateway/tests/session.rs` (`woken_task_continues_after_waker_replacement`); `AGENTS.md` "Источник истины для пакетов".
