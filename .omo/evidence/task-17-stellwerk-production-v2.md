# Task 17 (V2-13, F-12/F-13) — re-measure `audit_live` and the real-Pi budget

Plan item: todo 17 — "V2-13 F-12/F-13: re-measure audit_live and the real-Pi budget".

Scope: **measurement only**. Zero changes to `.rs`, `Cargo.toml`, `Cargo.lock`,
`BUILD_WINDOW`, `BUILD_BUDGET`, or `TEST_BUDGET_MS`. The only committed file is
this evidence file.

## Base

- Branch: `main`, worked directly in the main worktree (no worktree, no push).
- HEAD before this packet's commit: `756fab9` (`chore(evidence): attest V2/packet-12 after merge`).
- No CLI source changed since `7ac57cd` (see `learnings.md`, V2-10).
- Timer: `/usr/bin/time` is **not installed** on this host (`bash: /usr/bin/time: Datei oder
  Verzeichnis nicht gefunden`), so wall-clock was measured with `date +%s%N` arithmetic
  (the packet explicitly allows this). Timestamps are UTC (`date -u`); the host local
  zone is `Europe/Berlin` (CET, UTC+1).
- Each run was launched as its own process: `started=$(date -u ...)`, `date +%s%N`,
  the `cargo test` command, `date +%s%N`, `finished=$(date -u ...)`. Raw logs were kept
  under `/home/izyabretatel/.cache/task17/`.

## Suite facts (read before measuring, not modified)

- `crates/stellwerk-cli/tests/audit_live.rs:17` — `const TEST_BUDGET_MS: u64 = 50;` (unchanged).
  The file contains exactly **7** `#[test]` functions.
- `crates/stellwerk-cli/src/build_runner.rs:584-585` —
  `pub const BUILD_WINDOW: Duration = Duration::from_secs(60);`
  `pub const BUILD_BUDGET: Duration = Duration::from_secs(30);` (unchanged).
- `crates/stellwerk-cli/tests/build_runner.rs:185-197` — `PI_BIN` selects the launcher;
  without it the test falls back to `$HOME/.pi/agent/bin/pi`. The file contains exactly
  **17** `#[test]` functions.

## `audit_live` — six runs (3 serial, 3 parallel)

Command forms (recorded verbatim):

- serial:   `cargo test -p stellwerk-cli --test audit_live -- --test-threads=1`
- parallel: `cargo test -p stellwerk-cli --test audit_live`

| run | mode | command | started_at (UTC) | finished_at (UTC) | wall_clock_s | passed | failed | verbatim `test result:` line |
|----|------|---------|------------------|-------------------|--------------|--------|--------|------------------------------|
| 1 | serial   | `cargo test -p stellwerk-cli --test audit_live -- --test-threads=1` | 2026-11-19T00:18:16Z | 2026-11-19T00:18:16Z | 0.341 | 7 | 0 | `test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.19s` |
| 2 | serial   | `cargo test -p stellwerk-cli --test audit_live -- --test-threads=1` | 2026-11-19T00:18:16Z | 2026-11-19T00:18:17Z | 0.316 | 7 | 0 | `test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.18s` |
| 3 | serial   | `cargo test -p stellwerk-cli --test audit_live -- --test-threads=1` | 2026-11-19T00:18:17Z | 2026-11-19T00:18:17Z | 0.333 | 7 | 0 | `test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.19s` |
| 4 | parallel | `cargo test -p stellwerk-cli --test audit_live` | 2026-11-19T00:18:17Z | 2026-11-19T00:18:17Z | 0.197 | 7 | 0 | `test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s` |
| 5 | parallel | `cargo test -p stellwerk-cli --test audit_live` | 2026-11-19T00:18:17Z | 2026-11-19T00:18:18Z | 0.191 | 7 | 0 | `test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s` |
| 6 | parallel | `cargo test -p stellwerk-cli --test audit_live` | 2026-11-19T00:18:18Z | 2026-11-19T00:18:18Z | 0.195 | 7 | 0 | `test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s` |

All six runs: **7 passed / 0 failed / 0 ignored**, exit code 0. No retries were needed;
every serial and every parallel run passed on its first attempt.

Observed shape: serial ≈ 0.18–0.19 s of test time; parallel ≈ 0.06 s. That is the
expected direction for a suite whose `#[test]` fns each spawn independent fixture
servers and `unique_dir` keys on PID, so parallel threads do not contend on the same
tmpdir. Parallel is the risk case called out by the packet and it did **not** flake in
three consecutive runs.

## `build_runner` — real Pi 1.0.2

### Pi binary confirmation

```
$ /home/izyabretatel/.local/bin/pi --version
1.0.2

$ ls -l /home/izyabretatel/.local/bin/pi
lrwxrwxrwx ... /home/izyabretatel/.local/bin/pi -> ../../.pi/agent/bin/pi

$ readlink -f /home/izyabretatel/.local/bin/pi
/home/izyabretatel/.pi/agent/bin/pi

$ /home/izyabretatel/.pi/agent/bin/pi --version
1.0.2
```

Both paths are the same 1.0.2 binary; `.local/bin/pi` is a symlink into `.pi/agent/bin/pi`.

### Attempt A/B — the packet's literal `PI_BIN=/home/izyabretatel/.local/bin/pi` (FAILED, reproduced twice)

Command (verbatim):

```
PI_BIN=/home/izyabretatel/.local/bin/pi cargo test -p stellwerk-cli --test build_runner
```

| attempt | started_at (UTC) | finished_at (UTC) | wall_clock_s | passed | failed | verbatim `test result:` line |
|--------|------------------|-------------------|--------------|--------|--------|------------------------------|
| A | 2026-11-18T00:51:27Z | 2026-11-18T00:51:27Z | 0.543 | 13 | 4 | `test result: FAILED. 13 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s` |
| B | 2026-11-19T00:18:22Z | 2026-11-19T00:18:22Z | 0.142 | 13 | 4 | `test result: FAILED. 13 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s` |

The same 4 tests fail both times, with the identical panic text:

```
an_edit_reaches_the_copy_and_the_gate_authorizes_it ... FAILED
the_build_runs_pi_through_our_channels ... FAILED
candidate_contains_only_changed_workspace_files ... FAILED
the_build_logs_an_allowed_read ... FAILED
...
прогон: "запуск pi: process exited with code 1: bwrap: Can't find source path /home/izyabretatel/.local/install: No such file or directory"
```

This is **not** a budget/timeout failure and **not** flaky: it is deterministic. Cause:
`crates/stellwerk-cli/src/build_runner.rs:806-810` pushes `run.pi.parent()` and
`run.pi.parent().parent().join("install")` into the bwrap read-only binds. Given the
symlink path, the lexical parent is `/home/izyabretatel/.local/bin`, so the test asks
bwrap to bind `/home/izyabretatel/.local/install`, which does not exist. The real Pi
install root is `/home/izyabretatel/.pi/agent`, which *does* contain `install/`.

### Run C/D — real Pi via its canonical install root (PASSED)

To measure the real-Pi budget the test was designed for, the same 1.0.2 binary was
addressed by its canonical path (the path the test's own fallback at
`tests/build_runner.rs:188` uses):

```
PI_BIN=/home/izyabretatel/.pi/agent/bin/pi cargo test -p stellwerk-cli --test build_runner
```

| run | started_at (UTC) | finished_at (UTC) | wall_clock_s | passed | failed | verbatim `test result:` line |
|----|------------------|-------------------|--------------|--------|--------|------------------------------|
| C | 2026-11-19T00:17:55Z | 2026-11-19T00:17:57Z | 1.315 | 17 | 0 | `test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.93s` |
| D | 2026-11-19T00:18:26Z | 2026-11-19T00:18:27Z | 1.049 | 17 | 0 | `test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.92s` |

Run C is the official real-Pi measurement for this packet; run D re-confirms it after
the reproduced failure of A/B. Both: **17 passed / 0 failed / 0 ignored**, exit code 0.
No `--test-threads` flag was passed (the suite does not require it); the run was the
bare command shown above.

## Budget interpretation

- `TEST_BUDGET_MS = 50` (unchanged): every `audit_live` test completed well inside it in
  all six runs. The suite's own per-test budget is not stressed; the serial suite
  wall-clock (~0.19 s) and parallel (~0.06 s) are far below any plausible test timeout.
  **No budget change needed.**
- `BUILD_WINDOW = 60s` / `BUILD_BUDGET = 30s` (unchanged): the real-Pi suite finished in
  ~0.92–0.93 s of test time (1.05–1.32 s process wall-clock) for all 17 tests, including
  the 4 tests that actually start Pi 1.0.2 inside bwrap. This is ~1.5% of `BUILD_WINDOW`
  and ~3% of `BUILD_BUDGET`. **No budget change needed.** No failure ever reached either
  window/budget; the only failure was the path/bind error above, unrelated to timing.
- **No FINDING for flakiness was opened**: the six `audit_live` runs were all green and
  the real-Pi run was green. The one failure is the invocation-path defect recorded
  below.

## FINDING (recorded here, not fixed, no packet created)

**F-2026-11-19-01 — `PI_BIN` must name a launcher whose parent contains a sibling
`install/`; the `.local/bin/pi` symlink is not a valid `PI_BIN` for `build_runner`.**

- Observed: `PI_BIN=/home/izyabretatel/.local/bin/pi cargo test -p stellwerk-cli --test
  build_runner` fails deterministically with
  `bwrap: Can't find source path /home/izyabretatel/.local/install: No such
  file or directory` (13 passed / 4 failed, reproduced twice, attempts A and B above).
- Why: `build_runner.rs:806-810` derives read-only binds from the *lexical* parent of
  `run.pi`, not from the symlink target. `/home/izyabretatel/.local/bin/pi` is a symlink,
  so its lexical parent has no `install/` sibling.
- This packet's test instruction names `.local/bin/pi`; the suite's fallback and the
  working invocation use `/home/izyabretatel/.pi/agent/bin/pi`. The two point at the same
  Pi 1.0.2 binary; only the bind derivation differs.
- Classification: test-harness / invocation-path defect, **not** a budget defect. Per the
  packet this is a FINDING entry only; no `BUILD_WINDOW`/`BUILD_BUDGET`/`TEST_BUDGET_MS`
  edit, no separate packet file, no ADR. A future packet could make `run_build` canonicalize
  `run.pi` before deriving from `.parent()`, or bind `install` only when it exists and
  surface its absence as a named refusal (law 6), instead of passing a nonexistent source
  to bwrap.

## What was NOT done

- No `#[ignore]`, `#[allow]`, `--no-run`, `--skip`, or name filter was used.
- No source, test, build-script, manifest, or lockfile change.
- No rerun-until-green selection: every attempt above is recorded, including both failures.
- `.omo/plans/` and `.sisyphus/plans/` were not touched.
- The accepted dirty baseline (`.omo/omo.jsonc`, `opencode.json`, `tests/test_omo_config.py`,
  `.omo/archive/`, `.omo/drafts/`, `.playwright-mcp/`, `mutants.out*`) was not staged.

## PolyForge chain (V2/packet-13)

- Product commit: `bdd546a3e84c8ea389f3ba06d4cfed7ae4145e2f` (`test(cli): re-measure audit and real-Pi budgets`), 1 file, 173 insertions (evidence only; no code change).
- `diff_hash`: `d6d72e0e1adca62bb0313bed8ae0cc400951e6b63dba8f442190b02b751864d1`
- `evidence_append(ModelClaim)`: entry 408, hash `dd46d47da22ed8b83f5f335f26b88e3acdf91e53c826d2db6b2fa773967408cb`, state `ModelClaimed`.
- `evidence_verify(cargo test -p stellwerk-cli --test audit_live)`: entry 409, exit 0, state `Verified`, stdout_hash `dcca9a46b737b752584c520a5cfadcc09f2e978114e68869c00d338cec758e88`, hash `e7967dd7420e56a0a6409aa8d0ba5bf61070c10ac30ceac85fcb1b4fcb7082ec`.
- `gate_evaluate(required=[Verified])`: `passed=true`, verified=1, claimed=1, validated=0, missing=[].
- `chain_tail`: `e7967dd7420e56a0a6409aa8d0ba5bf61070c10ac30ceac85fcb1b4fcb7082ec`

Orchestrator independent verification: `git diff --stat 756fab9..HEAD -- '*.rs' '*.toml' 'Cargo.lock'` empty (zero code change); `cargo test -p stellwerk-cli --test audit_live -- --test-threads=1` -> 7 passed; `cargo test -p stellwerk-cli --test audit_live` -> 7 passed; `PI_BIN=/home/izyabretatel/.pi/agent/bin/pi cargo test -p stellwerk-cli --test build_runner` -> 17 passed; the literal `.local/bin/pi` invocation reproduced the bwrap failure with `Can't find source path /home/izyabretatel/.local/install`, confirming FINDING `F-2026-11-19-01`; `cargo fmt --all -- --check` exit 0; `cargo clippy -p stellwerk-cli --all-targets -- -D warnings` exit 0; commit contains exactly 1 file.
