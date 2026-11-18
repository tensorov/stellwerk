---
id: H1/05
milestone: H1
title: accept.sh reads canonical schema and RED/GREEN sections
depends_on: []
allowed_changes:
  - scripts/integrator/accept.sh
  - scripts/integrator/tests/accept_schema.sh
max_files: 2
max_diff_lines: 450
---

# H1/05 — accept.sh reads canonical schema and RED/GREEN sections

## Зачем

accept.sh reads canonical schema and RED/GREEN sections. This packet is a canonical H1 execution unit for repo-hygiene.

## RED

- `schema acceptance tests fail before adapter`

## Приёмка

```
bash scripts/integrator/tests/accept_schema.sh
```

## Не делается

No product code, no owner-WIP, no new dependencies, no `#[allow]`, no `#[ignore]`.

## Бюджет

C2: src ≤ 200 logical lines, tests ≤ 250 added lines. `max_diff_lines` is raw and non-authoritative.
