---
id: V2/12
task_id: V2-12
milestone: V2
status: proposed
depends_on: []
files_allowed: [.sisyphus/plans/H1/packet-09.md]
max_files: 3
max_diff_lines: 200
acceptance: cargo test -p xtask --test packet_replay_selftest
---

# V2/12 — F-06: commit H1/09 and mark it GREEN

## Цель
Цель: V2-12 F-06: commit H1/09 and mark it GREEN

## Шаг 1 — падающий тест
Не TDD (bookkeeping GREEN): проверка — `cargo test -p xtask --test packet_replay_selftest` проходит; `! rg -n "if path.parent.name in ('M1', 'M3')" scripts/packet_replay.py` succeeds.

## Что делать
stage `.sisyphus/plans/H1/packet-09.md`; set `status: GREEN` and add the evidence link to `5527e5d` and `cargo test -p xtask --test packet_replay_selftest`. The code fix is already in `5527e5d`; do not re-apply it. Must NOT claim GREEN without the test output. Must NOT change `scripts/packet_replay.py` again.

## Приёмка
`git status --porcelain .sisyphus/plans/H1/packet-09.md` empty; `cargo test -p xtask --test packet_replay_selftest` passes; `! rg -n "if path.parent.name in \('M1', 'M3'\)" scripts/packet_replay.py` succeeds.

## QA
happy - the test and the `rg`; failure - the old skip would make `m1_and_m3_packets_are_not_filtered` fail. Evidence `.omo/evidence/task-16-stellwerk-production-v2.md`.

## Запрещено
- do not re-apply it.
- Must NOT claim GREEN without the test output.
- Must NOT change `scripts/packet_replay.py` again.

## Ссылки
`.sisyphus/plans/H1/packet-09.md`; `git show --stat 5527e5d`; `scripts/packet_replay.py`; `xtask/tests/packet_replay_selftest.rs` (`m1_and_m3_packets_are_not_filtered`); `.omo/evidence/final-wave-80ed4c1/f1-replay-validate.log`.
