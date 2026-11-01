# P00 — measured baseline

**Measured:** 2026-11-01.
**Status:** dated snapshot of the repository *before* P01 (task 3). It is not a
permanent invariant: the `fmt` and `clippy` rows below are expected to change when
P01 lands, and `plan-lint` stays RED until L01.

**Raw evidence:** `/tmp/opencode/p00/`. Commands are quoted as `just` invokes them
(`justfile:5,8,72,76`). The workspace test row uses the command behind
`just test-all` (`justfile:69`), because the justfile has no plain `test` target.

## The six measured commands

| id | command | measured | key output |
| --- | --- | --- | --- |
| fmt | `cargo fmt --all -- --check` | exit 1 | 12 diff hunks in 4 files (`fmt.txt`) |
| clippy | `cargo clippy --workspace --all-targets -- -D warnings` | exit 101 | `error: manual implementation of a no-op waker` at `crates/stellwerk-gateway/src/session.rs:613` (`clippy.txt:5-18`) |
| test | `cargo test --workspace --no-fail-fast` | exit 0 | 650 passed, 0 failed across the six crates (`test-exits.txt` + `test-stellwerk-*.txt`) |
| plan-lint | `just plan-lint` | exit 1, 77 violations — RED | 54 `[C1]` + 23 `[C9]` (`planlint-full.txt`, `plan-lint.txt`) |
| replay | `python3 scripts/packet_replay.py inventory` | exit 0 | `total=50`, `PARSED=49`, `NO_COMMAND=1`, `PARSE_ERROR=0` (`replay-inventory.txt:51-54`) |
| doctor | `just doctor` | exit 0 | `.sisyphus/plans: ok`, `subagent_depth=1: ok` (`doctor.txt:5-6`) |

### fmt — exit 1

Evidence: `/tmp/opencode/p00/fmt.txt`. Four files are unformatted:
`crates/stellwerk-gateway/tests/session_failure.rs`,
`crates/stellwerk-gateway/tests/shaping.rs`,
`crates/stellwerk-task/tests/http_wire.rs`,
`crates/stellwerk-task/tests/model_backend.rs`.
`session_failure.rs` and `shaping.rs` are owner WIP and must not be rewritten by
P00; P01 owns the formatting fix.

### clippy — exit 101

Evidence: `/tmp/opencode/p00/clippy.txt`. One error:

```text
error: manual implementation of a no-op waker
   --> crates/stellwerk-gateway/src/session.rs:613:10
```

The captured file ends with `CLIPPY_EXIT=101`.

### test — exit 0

Evidence: `/tmp/opencode/p00/test-exits.txt` records one `EXIT=0` line for each of
the six crates and then `DONE`. The per-crate output files sum to **650 passed,
0 failed**:

| crate | passed |
| --- | --- |
| `stellwerk-registry` | 81 |
| `stellwerk-core` | 63 |
| `stellwerk-store` | 27 |
| `stellwerk-task` | 294 |
| `stellwerk-gateway` | 90 |
| `stellwerk-cli` | 95 |

**Discrepancy recorded, not hidden:** the plan/task text says `cargo test` is
`0, 99 tests`. No file under `/tmp/opencode/p00/` contains that `99`; the captured
per-crate outputs sum to 650. This document uses the measured 650 and treats the
`99` as a stale figure in the plan text.

### plan-lint — exit 1, 77 violations (RED)

Evidence: `/tmp/opencode/p00/planlint-full.txt`; the wrapper run in
`plan-lint.txt` ends with `error: recipe 'plan-lint' failed on line 73 with exit code 1`.
The 77 violations break down as:

- `N1`: 20 `[C1]` (missing `depends_on` links between packets sharing files);
- `N2`: 32 `[C1]`;
- `TUI`: 2 `[C1]`;
- 23 `[C9]` cross-milestone pairs (`ПЕРЕСЕЧЕНИЯ МЕЖДУ MILESTONE: 23`).

`H1` and `M1` report zero violations; `ЗЕРКАЛО ПАКЕТОВ` matches the canon and
`ТАБЛИЦА risk_class` matches `contract.rs`. The captured run predates V1 packet
materialization (its headers are H1/M1/N1/N2/TUI only).

**`plan-lint` is RED at this baseline. This is not an all-green baseline.**
Closing those 77 violations is L01, not P00.

### replay — exit 0

Evidence: `/tmp/opencode/p00/replay-inventory.txt`. The tail is:

```text
total=50
PARSED=49
NO_COMMAND=1
PARSE_ERROR=0
```

The single `NO_COMMAND` row is `N2/00i1`.

### doctor — exit 0

Evidence: `/tmp/opencode/p00/doctor.txt`: `opencode: 1.19.5`, `polyforge` found,
`gh version 2.102.1`, `cargo 1.99.0`, `.sisyphus/plans: ok`,
`subagent_depth=1: ok`. `just doctor` does not report Pi; the Pi version is taken
from the probe below.

## Workspace crates

The six measured workspace crates are:

- `stellwerk-core`
- `stellwerk-gateway`
- `stellwerk-registry`
- `stellwerk-store`
- `stellwerk-task`
- `stellwerk-cli`

`xtask` is also listed in `[workspace] members` but is an auxiliary workspace
tool, not one of the six runtime crates measured above.

## Installed Pi version

The installed Pi is **0.87.0**. Source: `docs/PI_1_0_2_PROBE.md:10` —
`pi --version` → `0.87.0` (`~/.pi/agent/bin/pi`). Pi 1.0.2 is the target the V1
plan works against but is not installed on this machine.

## What this document is not

It is a dated measurement, not a contract. P01 changes the `fmt`/`clippy` rows;
L01 changes the `plan-lint` row. Re-measure rather than editing numbers by hand.
