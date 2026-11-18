---
id: H1/08
milestone: H1
title: record repo-hygiene findings and exact inventory
depends_on: []
allowed_changes:
  - .sisyphus/plans/H1/FINDINGS.md
max_files: 1
max_diff_lines: 450
---

# H1/08 — record repo-hygiene findings and exact inventory

## Зачем

record repo-hygiene findings and exact inventory. This packet is a canonical H1 execution unit for repo-hygiene.

## RED

- `findings file is absent`

## Приёмка

```
test -s .sisyphus/plans/H1/FINDINGS.md
```

## Не делается

No product code, no owner-WIP, no new dependencies, no `#[allow]`, no `#[ignore]`.

## Бюджет

C2: src ≤ 200 logical lines, tests ≤ 250 added lines. `max_diff_lines` is raw and non-authoritative.
