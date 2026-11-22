---
id: H1/06
milestone: H1
title: restore the three C7 source documents verbatim
depends_on: []
allowed_changes:
  - .sisyphus/plans/M1/packet-05.md
  - .sisyphus/plans/M1/packet-06.md
  - .sisyphus/plans/M3/REVIEW.md
max_files: 3
max_diff_lines: 450
---

# H1/06 — restore the three C7 source documents verbatim

## Зачем

restore the three C7 source documents verbatim. This packet is a canonical H1 execution unit for repo-hygiene.

## RED

- `C7 gating fails while documents are absent`

## Приёмка

```
python3 scripts/plan-lint.py --gating
```

## Не делается

No product code, no owner-WIP, no new dependencies, no `#[allow]`, no `#[ignore]`.

## Бюджет

C2: src ≤ 200 logical lines, tests ≤ 250 added lines. `max_diff_lines` is raw and non-authoritative.
