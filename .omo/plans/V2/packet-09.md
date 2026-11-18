---
id: V2/09
task_id: V2-09
milestone: V2
status: proposed
depends_on: []
files_allowed: [docs/adr/0015-execution-spec-not-task-spec.md]
max_files: 3
max_diff_lines: 200
acceptance: 'rg -n "^Статус: отменён \(superseded by ADR-0021\)$" docs/adr/0015-execution-spec-not-task-spec.md'
---

# V2/09 — F-10: mark ADR-0015 superseded by ADR-0021

## Цель
Цель: V2-09 F-10: mark ADR-0015 superseded by ADR-0021

## Шаг 1 — падающий тест
Не TDD (docs/ADR): проверка — `rg -n "^Статус: отменён \(superseded by ADR-0021\)$" docs/adr/0015-execution-spec-not-task-spec.md`.

## Что делать
change only the status line in `docs/adr/0015-execution-spec-not-task-spec.md` from `Статус: принят` to `Статус: отменён (superseded by ADR-0021)`; add the superseding link if the ADR index has one. Must NOT rewrite the ADR body or history. Must NOT change ADR-0021.

## Приёмка
`rg -n "^Статус: отменён \(superseded by ADR-0021\)$" docs/adr/0015-execution-spec-not-task-spec.md` matches; `git diff -- docs/adr/0015-execution-spec-not-task-spec.md` shows only the status line (and a link if needed).

## QA
happy - the `rg` check; failure - `rg "Статус: принят" docs/adr/0015...` must not match. Evidence `.omo/evidence/task-13-stellwerk-production-v2.md`.

## Запрещено
- Must NOT rewrite the ADR body or history.
- Must NOT change ADR-0021.

## Ссылки
`docs/adr/0015-execution-spec-not-task-spec.md:3`; `docs/adr/0021-single-execution-profile.md:1-10`; `docs/adr/README.md` if it exists; `AGENTS.md` law 5 (ADR for contract changes).
