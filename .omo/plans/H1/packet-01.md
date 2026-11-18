---
id: H1/01
milestone: H1
title: plan-lint parses N2 YAML packets
depends_on: []
allowed_changes:
  - scripts/plan-lint.py
  - scripts/tests/test_plan_lint_formats.py
max_files: 2
max_diff_lines: 450
---

# H1/01 — plan-lint parses N2 YAML packets

## Зачем

plan-lint parses N2 YAML packets. This packet is a canonical H1 execution unit for repo-hygiene.

## RED

- `n2 parser tests fail before adapter`

## Приёмка

```
python3 -m unittest discover -s scripts/tests -p 'test_plan_lint_formats.py' -v
```

## Не делается

No product code, no owner-WIP, no new dependencies, no `#[allow]`, no `#[ignore]`.

## Бюджет

C2: src ≤ 200 logical lines, tests ≤ 250 added lines. `max_diff_lines` is raw and non-authoritative.
