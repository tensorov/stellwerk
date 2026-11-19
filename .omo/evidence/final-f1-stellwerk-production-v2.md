# F1 — Plan compliance audit: `stellwerk-production-v2`

- Plan: `.omo/plans/stellwerk-production-v2.md`
- Repo: `/home/izyabretatel/gits/stellwerk` — branch `main`, HEAD `5d7e50bf2e4f64cd09133d088f009eaa26babae9`
- Run (UTC): `2026-11-19T00:27:37Z`
- Reviewer mode: read-only. Only this evidence file was written; no source/test/plan/packet/ledger/boulder/notepad file touched, no commit, no PolyForge call.

## 1. Per-todo acceptance evidence (todos 1–18)

Evidence file naming check: all 18 files exist (`ls -1 .omo/evidence/ | grep -c 'stellwerk-production-v2'` → `18`), names `.omo/evidence/task-<N>-stellwerk-production-v2.md` for N=1..18. None missing.

| # | Evidence file | Acceptance command(s) named in the todo | Matching line(s) in the evidence file | Result |
|---|---------------|------------------------------------------|----------------------------------------|--------|
| 1 | `.omo/evidence/task-1-stellwerk-production-v2.md` | `git status --porcelain .sisyphus/plans/V1`; `python3 scripts/plan-lint.py .sisyphus/plans/V1`; `python3 scripts/packet_replay.py validate --root .sisyphus/plans --only V1`; `python3 scripts/plan-lint.py --self-test`; `ls .sisyphus/plans/V2/packet-*.md`; `rg -n "contract-change" .sisyphus/plans/V2/packet-07a.md`; `python3 scripts/plan-lint.py .sisyphus/plans/V2` | `\| 1 \| git status --porcelain .sisyphus/plans/V1 \| 0 \| empty ...` / `\| 4 \| python3 scripts/plan-lint.py .sisyphus/plans/V1 \| 0 \| V1: 34 пакетов, нарушений нет` / `\| 5 \| python3 scripts/packet_replay.py validate --root .sisyphus/plans --only V1 \| 0 \| METADATA_PACKAGES=7` / `\| 6 \| python3 scripts/plan-lint.py --self-test \| 0 \| self-test: провалов 0 из 47 проверок` / `\| 2 \| ls .sisyphus/plans/V2/packet-*.md \| wc -l \| 0 \| 17` / `\| 7 \| rg -l "contract-change" .sisyphus/plans/V2 \| 0 \| only .sisyphus/plans/V2/packet-07a.md` / `\| 3 \| python3 scripts/plan-lint.py .sisyphus/plans/V2 \| 0 \| V2: 17 пакетов, нарушений нет` | PASS (command 2 run with `\| wc -l`; command 7 run as `rg -l` scoped to `V2` — both actual runs, result recorded) |
| 2 | `.omo/evidence/task-2-stellwerk-production-v2.md` | `cargo test -p stellwerk-cli --test run`; `cargo test -p stellwerk-task --test evidence` | `cargo test -p stellwerk-cli --test run` (line 105) with `test run_attestation_carries_bindings ... ok` / `test run_without_git_head_refuses ... ok`; `cargo test -p stellwerk-task --test evidence` (line 130) | PASS |
| 3 | `.omo/evidence/task-3-stellwerk-production-v2.md` | `cargo test -p stellwerk-task --test lifecycle --test suspension`; `cargo test -p stellwerk-cli --test vertical_v4` | `cargo test -p stellwerk-task --test lifecycle --test suspension` (lines 12, 54); `cargo test -p stellwerk-cli --test vertical_v4` (line 94) | PASS |
| 4 | `.omo/evidence/task-4-stellwerk-production-v2.md` | `cargo test -p stellwerk-task --test human_validation --test runner` | `cargo test -p stellwerk-task --test human_validation --test runner` (lines 13, 64) | PASS |
| 5 | `.omo/evidence/task-5-stellwerk-production-v2.md` | `cargo test -p stellwerk-cli --test plan` | `cargo test -p stellwerk-cli --test plan build_status_names_every_decision` (lines 9, 31); `cargo test -p stellwerk-cli --test plan` (line 49) | PASS |
| 6 | `.omo/evidence/task-6-stellwerk-production-v2.md` | `cargo test -p stellwerk-cli --test run` | `cargo test -p stellwerk-cli --test run` (lines 11, 35) with named tests ok | PASS |
| 7 | `.omo/evidence/task-7-stellwerk-production-v2.md` | `cargo test -p stellwerk-cli --test run`; `PI_BIN=<real-pi-1.0.2> cargo test -p stellwerk-cli --test run run_task_with_real_pi` | `cargo test -p stellwerk-cli --test run` (lines 11, 50, 13 passed); `pi --version` → `1.0.2`; `cargo test -p stellwerk-cli --test run run_task_with_real_pi -- --nocapture` → `real Pi ... -> 1.0.2` / `test ... ok` (lines 90–96) | PASS |
| 8 | `.omo/evidence/task-8-stellwerk-production-v2.md` | `cargo test -p stellwerk-task --test taskfile --test execution`; `cargo test -p stellwerk-cli --test run`; `rg -n "execution::resolve\|task_file\.resolve" ...` | `cargo test -p stellwerk-task --test taskfile --test execution` (line 55); `cargo test -p stellwerk-cli --test run` (line 76, 13 passed); `$ rg -n "execution::resolve" ...` (107), `$ rg -n "task_file\.resolve\|resolved\.program" ...` (112) | PASS |
| 9 | `.omo/evidence/task-9-stellwerk-production-v2.md` | `cargo test -p stellwerk-core`; `cargo check -p stellwerk-task`; `test -f docs/adr/0024-refutation-evidence-kind.md`; `rg -n "contract-change" .sisyphus/plans/V2/packet-07a.md` | `$ cargo test -p stellwerk-core` (lines 26, 54, EXIT=0); `$ cargo check --workspace --all-targets` → `CHECK_EXIT=0` (line 92) — strict superset of the named package check; ADR existence documented as `docs/adr/0024-refutation-evidence-kind.md (110 lines)` (line 19) and in C2 numstat (`110/0`, line 151) in lieu of literal `test -f`; `$ rg -n "contract-change: true" .sisyphus/plans/V2/packet-07a.md` → `11:contract-change: true` (line 15) | PASS (workspace-wide `cargo check` proves the named package; ADR presence proven by line count/numstat, not a literal `test -f` run) |
| 10 | `.omo/evidence/task-10-stellwerk-production-v2.md` | `cargo test -p stellwerk-task --test evidence` | `Command: cargo test -p stellwerk-task --test evidence` (lines 9, 29); result `22 passed` (line 85) | PASS |
| 11 | `.omo/evidence/task-11-stellwerk-production-v2.md` | `cargo test -p stellwerk-task --test evidence --test refutation` | `cargo test -p stellwerk-task --test refutation` (line 14); `cargo test -p stellwerk-task --test evidence --test refutation` (line 60); `22 + 7 passed` (line 133) | PASS |
| 12 | `.omo/evidence/task-12-stellwerk-production-v2.md` | `python3 scripts/plan-lint.py` reports `ЗЕРКАЛО ПАКЕТОВ: совпадает с каноном`; `diff -rq` V2/V1/N2 exit 0; `git ls-files .omo/plans`; `python3 scripts/plan-lint.py .sisyphus/plans/V1` | C8 AFTER block ends `ЗЕРКАЛО ПАКЕТОВ: совпадает с каноном` (lines 51–64); `diff -rq .sisyphus/plans/V2 .omo/plans/V2 -> exit 0` / V1 / N2 (lines 73–75); `python3 scripts/plan-lint.py .sisyphus/plans/V1 -> exit 0` (line 76); `git ls-files .omo/plans \| wc -l = 126` / `grep 'V2/' ... = 17` / `git status --porcelain .omo/plans -> (empty)` (lines 219–221); summary `python3 scripts/plan-lint.py -> ... exit 0` (line 263) | PASS |
| 13 | `.omo/evidence/task-13-stellwerk-production-v2.md` | `rg -n "^Статус: отменён \(superseded by ADR-0021\)$" docs/adr/0015-execution-spec-not-task-spec.md`; `git diff -- docs/adr/0015-execution-spec-not-task-spec.md` | `$ rg -n '^Статус: отменён \(superseded by ADR-0021\)$' docs/adr/0015-execution-spec-not-task-spec.md` → `3:Статус: отменён (superseded by ADR-0021)` / `EXIT=0` (lines 26–28); `$ git diff -- docs/adr/0015-execution-spec-not-task-spec.md` with only the status line changed (lines 43–58) | PASS (docs-only/ADR packet; evidence honestly declares `passed=false missing:["Verified"]` per ADR-0006 — declared boundary, presence criterion met) |
| 14 | `.omo/evidence/task-14-stellwerk-production-v2.md` | `! rg -n "Waker::noop" .sisyphus/plans/V1/packet-01.md`; `rg -n "Unpark\|park" .sisyphus/plans/V1/packet-01.md`; `cargo test -p stellwerk-gateway --test session` | `$ ! rg -n "Waker::noop" ...` → `NO_MATCH_EXIT=0` (line 26); `$ rg -n "Unpark\|park" ...` → matches lines 15, 18 (lines 29–31); `$ cargo test -p stellwerk-gateway --test session` → `5 passed; 0 failed` (lines 34–46) | PASS |
| 15 | `.omo/evidence/task-15-stellwerk-production-v2.md` | `cargo test -p stellwerk-task --test ledger_sqlite` | `$ cargo test -p stellwerk-task --test ledger_sqlite old_ledger` (line 14); `### cargo test -p stellwerk-task --test ledger_sqlite` (line 36); `7 passed 0 failed` (line 335) | PASS |
| 16 | `.omo/evidence/task-16-stellwerk-production-v2.md` | `git status --porcelain .sisyphus/plans/H1/packet-09.md` empty; `cargo test -p xtask --test packet_replay_selftest`; `! rg -n "if path.parent.name in \('M1', 'M3'\)" scripts/packet_replay.py` | `git status --porcelain .sisyphus/plans/H1/packet-09.md` → empty (line 197); `cargo test -p xtask --test packet_replay_selftest` (line 47, 10 passed); `rg -n "if path.parent.name in" scripts/packet_replay.py` → `exit 1` (lines 33, 159, 197) | PASS (fabricated-attestation incident later corrected under V2/packet-12; presence criterion met) |
| 17 | `.omo/evidence/task-17-stellwerk-production-v2.md` | 3× `cargo test -p stellwerk-cli --test audit_live -- --test-threads=1`; 3× `cargo test -p stellwerk-cli --test audit_live`; `PI_BIN=<pi-1.0.2> cargo test -p stellwerk-cli --test build_runner` | Rows 1–3 serial `7 passed`, rows 4–6 parallel `7 passed` (lines 42–47); `PI_BIN=... cargo test -p stellwerk-cli --test build_runner` (lines 83, 116) `17 passed` (line 184) | PASS |
| 18 | `.omo/evidence/task-18-stellwerk-production-v2.md` | `cargo test -p stellwerk-cli --test candidate`; enum-shape check | `Command: cargo test -p stellwerk-cli --test candidate symlink_target_change_is_dirty` (line 8); `cargo test -p stellwerk-cli --test candidate` (line 131) `8 passed / 0 failed` (line 207); `rg -n "enum CandidateVerdict" -A 5 ...` (line 187) | PASS |

No todo FAILs. Two substantive-but-superset substitutions are noted explicitly in row 9 (`cargo check --workspace --all-targets` covers `cargo check -p stellwerk-task`; ADR existence proven by documented 110-line file + C2 numstat rather than a literal `test -f`). Neither weakens the acceptance conclusion; both are recorded rather than silently accepted.

## 2. `python3 scripts/plan-lint.py` (raw, from repo root)

```
H1: 9 пакетов, нарушений нет
M1: 2 пакетов, нарушений нет
N1: 10 пакетов, нарушений нет
N2: 27 пакетов, нарушений нет
TUI: 5 пакетов, нарушений нет
V1: 34 пакетов, нарушений нет
V2: 17 пакетов, нарушений нет
ЗЕРКАЛО ПАКЕТОВ: совпадает с каноном
ПЕРЕСЕЧЕНИЯ МЕЖДУ MILESTONE: порядок объявлен везде
ТАБЛИЦА risk_class: совпадает с contract.rs во всех трёх документах
EXIT=0
```

- Exit code: `0`.
- C1–C9 violations: **none**. No `C1`…`C9` line appears in the output.
- Exact mirror line: `ЗЕРКАЛО ПАКЕТОВ: совпадает с каноном` → gate satisfied.

## 3. `git status --porcelain .sisyphus/plans/V2 .omo/plans` (raw)

```
 M .omo/plans/stellwerk-production-v2.md
```

(Also re-run with `--untracked-files=all`: identical single line. No untracked files.)

Line-by-line judgment:

| Line | Artifact | Judgment |
|------|----------|----------|
| ` M .omo/plans/stellwerk-production-v2.md` | Orchestrator state — checkbox ticks for the closed todos + FINAL-WAVE items | **Expected, not a violation.** This is the orchestrator's plan copy, not a packet; it is the documented mutable orchestrator-state file. |

- `.sisyphus/plans/V2`: clean — no packet drift.
- Committed mirror packets under `.omo/plans`: clean.
- No unexpected V2/mirror files.
- Supplementary `diff -rq` (beyond F1's required command, for confirmation): `.sisyphus/plans/V2` vs `.omo/plans/V2` → exit 0; `V1` → exit 0; `N2` → exit 0.

F1 scope is `.sisyphus/plans/V2` and `.omo/plans` only; the accepted dirty baseline (`.omo/omo.jsonc`, `opencode.json`, `tests/test_omo_config.py`, `.playwright-mcp/`, `mutants.out*`, etc.) and the out-of-scope untracked `.sisyphus/plans/H1/packet-01..08.md` are deliberately not judged here.

## 4. Plan checkbox counts

```
grep -c '^- \[x\]' .omo/plans/stellwerk-production-v2.md  -> 18
grep -c '^- \[ \]' .omo/plans/stellwerk-production-v2.md  -> 4
```

18 completed implementation todos; the 4 open items are exactly F1, F2, F3, F4 (FINAL-WAVE). Matches the expected state.

## Verdict

- All 18 implementation todos have an existing evidence file whose recorded command run satisfies the todo's acceptance criteria (with row-9 superset substitutions noted and accepted).
- `python3 scripts/plan-lint.py` exits `0`, reports no C1–C9 violations, and prints `ЗЕРКАЛО ПАКЕТОВ: совпадает с каноном`.
- Scoped `git status` shows only the orchestrator-state plan file ` M .omo/plans/stellwerk-production-v2.md`; no V2 packet drift and no unexplained mirror artifacts.
- Checkbox counts: 18 done / 4 open FINAL-WAVE (F1–F4), as expected.

F1 PASS. No finding raised.
