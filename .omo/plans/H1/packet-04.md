---
id: H1/04
milestone: H1
title: packet_replay reads canonical N2/N1/TUI formats
depends_on: []
allowed_changes:
  - scripts/packet_replay.py
  - scripts/tests/test_packet_replay_formats.py
max_files: 2
max_diff_lines: 450
---

# H1/04 — packet_replay reads canonical N2/N1/TUI formats

## Зачем

packet_replay reads canonical N2/N1/TUI formats. This packet is a canonical H1 execution unit for repo-hygiene.

## RED

- `format extraction tests fail before adapter`

## Приёмка

```
python3 -m unittest discover -s scripts/tests -p 'test_packet_replay_formats.py' -v
```

## Не делается

No product code, no owner-WIP, no new dependencies, no `#[allow]`, no `#[ignore]`.

## Бюджет

C2: src ≤ 200 logical lines, tests ≤ 250 added lines. `max_diff_lines` is raw and non-authoritative.
