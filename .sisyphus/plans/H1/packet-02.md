---
id: H1/02
milestone: H1
title: plan-lint parses N1 and TUI packets
depends_on: [H1/01]
allowed_changes:
  - scripts/plan-lint.py
  - scripts/tests/test_plan_lint_formats.py
max_files: 2
max_diff_lines: 450
---

# H1/02 — plan-lint parses N1 and TUI packets

## Зачем

plan-lint parses N1 and TUI packets. This packet is a canonical H1 execution unit for repo-hygiene.

## RED

- `n1/tui parser tests fail before adapter`

## Приёмка

```
python3 -m unittest discover -s scripts/tests -p 'test_plan_lint_formats.py' -v
```

## Не делается

No product code, no owner-WIP, no new dependencies, no `#[allow]`, no `#[ignore]`.

## Бюджет

C2: src ≤ 200 logical lines, tests ≤ 250 added lines. `max_diff_lines` is raw and non-authoritative.
