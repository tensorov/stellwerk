---
id: H1/07
milestone: H1
title: track the completed canonical 00j7a packet
depends_on: []
allowed_changes:
  - .sisyphus/plans/N2/packet-00j7a.md
max_files: 1
max_diff_lines: 450
---

# H1/07 — track the completed canonical 00j7a packet

## Зачем

track the completed canonical 00j7a packet. This packet is a canonical H1 execution unit for repo-hygiene.

## RED

- `00j7a is untracked`

## Приёмка

```
git ls-files --error-unmatch .sisyphus/plans/N2/packet-00j7a.md
```

## Не делается

No product code, no owner-WIP, no new dependencies, no `#[allow]`, no `#[ignore]`.

## Бюджет

C2: src ≤ 200 logical lines, tests ≤ 250 added lines. `max_diff_lines` is raw and non-authoritative.
