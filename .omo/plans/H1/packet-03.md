---
id: H1/03
milestone: H1
title: plan-lint C9 sees current milestones and C7 self-test is green
depends_on: [H1/02]
allowed_changes:
  - scripts/plan-lint.py
  - scripts/tests/test_plan_lint_formats.py
max_files: 2
max_diff_lines: 450
---

# H1/03 — plan-lint C9 sees current milestones and C7 self-test is green

## Зачем

plan-lint C9 sees current milestones and C7 self-test is green. This packet is a canonical H1 execution unit for repo-hygiene.

## RED

- `c9 and c7 fixture tests fail before fix`

## Приёмка

```
python3 -m unittest discover -s scripts/tests -p 'test_plan_lint_formats.py' -v
```

## Не делается

No product code, no owner-WIP, no new dependencies, no `#[allow]`, no `#[ignore]`.

## Бюджет

C2: src ≤ 200 logical lines, tests ≤ 250 added lines. `max_diff_lines` is raw and non-authoritative.
