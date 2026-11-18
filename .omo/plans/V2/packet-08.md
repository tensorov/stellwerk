---
id: V2/08
task_id: V2-08
milestone: V2
status: proposed
depends_on: []
files_allowed: [.omo/plans/**]
max_files: 3
max_diff_lines: 200
acceptance: python3 scripts/plan-lint.py
---

# V2/08 — F-09: sync the .omo/plans mirror with canon

## Цель
Цель: V2-08 F-09: sync the .omo/plans mirror with canon

## Шаг 1 — падающий тест
Не TDD (bookkeeping mirror): проверка — `python3 scripts/plan-lint.py` печатает `ЗЕРКАЛО ПАКЕТОВ: совпадает с каноном`, плюс `diff -rq` по V2/V1/N2.

## Что делать
re-run `python3 scripts/plan-lint.py`, take the current C8 list as the source of truth, and copy each listed canonical packet from `.sisyphus/plans/` to `.omo/plans/` byte-for-byte; explicitly copy the V2 tree with `cp -a .sisyphus/plans/V2/. .omo/plans/V2/` (C8 cannot see canon-only packets, so it cannot prove the V2 copy), then verify with `diff -rq .sisyphus/plans/V2 .omo/plans/V2`; do the same `diff -rq` for V1 and N2; archive removed versions under `.omo/plans/_archive/`; do not edit canonical packets. For every mirror packet with no canonical counterpart, move it to `.omo/plans/_archive/`; do not delete canonical packets. `.omo/plans` is untracked, so `git diff --stat .omo/plans` is empty; use `git add -A .omo/plans` (bookkeeping, not a product commit) and verify with `git diff --cached --name-only .omo/plans` before the commit and `git ls-files .omo/plans` after. Do not hard-code the prior count: P0 or later edits may change the drift set. Mirror sync is `.omo/plans/**` bookkeeping, not a product packet; the C2 diff budget does not apply. Must NOT edit canon to match the mirror.

## Приёмка
`python3 scripts/plan-lint.py` reports `ЗЕРКАЛО ПАКЕТОВ: совпадает с каноном`; `diff -rq .sisyphus/plans/V2 .omo/plans/V2`, `diff -rq .sisyphus/plans/V1 .omo/plans/V1`, and `diff -rq .sisyphus/plans/N2 .omo/plans/N2` exit 0; `git diff --cached --name-only .omo/plans` lists only the files named by the current C8 output and archives (or check `git status --porcelain .omo/plans` after committing); `git ls-files .omo/plans` includes the synced files; `python3 scripts/plan-lint.py .sisyphus/plans/V1` still exits 0.

## QA
happy - `python3 scripts/plan-lint.py` and `git status --porcelain .omo/plans`; failure - intentionally leave one drift and assert C8 reports it, then fix it. Evidence `.omo/evidence/task-12-stellwerk-production-v2.md`.

## Запрещено
- do not edit canonical packets.
- do not delete canonical packets.
- Do not hard-code the prior count: P0 or later edits may change the drift set.
- Must NOT edit canon to match the mirror.

## Ссылки
`scripts/plan-lint.py:596-650` (`check_mirror_drift`); `.omo/evidence/final-wave-80ed4c1/f1-plan-lint.log` (the last recorded C8 list); `.omo/plans/N2/`; `.omo/plans/V1/`; `.sisyphus/plans/N2/`; `.sisyphus/plans/V1/`; AGENTS.md "Источник истины для пакетов".
