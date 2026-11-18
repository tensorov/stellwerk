# Task 16 (V2-12, F-06) — H1/09 marked GREEN

Packet: `H1/09` — "packet_replay scans M1/M3 instead of hiding them".

Scope: packet bookkeeping only. No product code, no test code, no
`scripts/packet_replay.py` change. The fix is already committed on `main` as
`5527e5d9f5ed22f3816bf2b23254711e098ce494`.

## Base

- Branch: `main`
- HEAD before the bookkeeping commit: `b6d432e` (`chore(evidence): attest V2/packet-11 after merge`)
- `5527e5d` is an ancestor of HEAD.

## Code fix already shipped (unchanged by this task)

```
$ git show --stat 5527e5d
commit 5527e5d9f5ed22f3816bf2b23254711e098ce494
Author: tensorov <aleksei.v.molchanov@gmail.com>
Date:   Mon Nov 2 18:18:52 2026 +0100

    fix(replay): scan M1/M3 packets instead of hiding them

 scripts/packet_replay.py              |  3 ---
 xtask/tests/packet_replay_selftest.rs | 31 +++++++++++++++++++++++++++++++
 2 files changed, 31 insertions(+), 3 deletions(-)
```

Skip branch is gone:

```
$ rg -n "if path.parent.name in" scripts/packet_replay.py
$ echo $?
1
```

## Acceptance — verbatim

Build environment note: `/tmp` is a 16G tmpfs with ~14G free, but the packet
records ENOSPC failures on it, so both runs used `TMPDIR` under `/home` and a
separate `CARGO_TARGET_DIR`:

```
TMPDIR=/home/izyabretatel/.cache/stellwerk-tmp \
CARGO_TARGET_DIR=/home/izyabretatel/.cache/stellwerk-target \
cargo test -p xtask --test packet_replay_selftest
```

```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.31s
     Running tests/packet_replay_selftest.rs (/home/izyabretatel/.cache/stellwerk-target/debug/deps/packet_replay_selftest-2471edff9e26c4bd)

running 10 tests
test source_uses_no_cut_and_no_shell ... ok
test omo_mirror_is_not_scanned ... ok
test prose_and_code_block_mentions_are_not_fields ... ok
test long_target_name_is_not_truncated ... ok
test m1_and_m3_packets_are_not_filtered ... ok
test counts_match_fixture_not_brief_heuristics ... ok
test backticked_command_is_parsed ... ok
test single_line_test_field_is_parsed ... ok
test packet_without_test_field_is_no_command ... ok
test archive_is_not_scanned ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
```

Exit status: 0. `m1_and_m3_packets_are_not_filtered` is present.

```
TMPDIR=/home/izyabretatel/.cache/stellwerk-tmp \
CARGO_TARGET_DIR=/home/izyabretatel/.cache/stellwerk-target \
cargo test -p xtask --test packet_replay_validate
```

```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.21s
     Running tests/packet_replay_validate.rs (/home/izyabretatel/.cache/stellwerk-target/debug/deps/packet_replay_validate-bb6f6e190a3ebbbe)

running 5 tests
test positional_filter_is_not_a_target ... ok
test nonexistent_package_is_invalid_package ... ok
test test_target_is_validated_against_metadata ... ok
test nonexistent_target_is_invalid_target ... ok
test shell_and_escape_attempts_are_unsupported ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.87s
```

Exit status: 0.

## plan-lint

```
$ python3 scripts/plan-lint.py
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
```

Exit status: 0. C8 reports no drift; `status` is not one of the compared fields
(`depends_on` / `files_allowed` / `goal` / `acceptance`), so GREEN on canon needs
no mirror edit. `## Зачем` is byte-identical to the mirror's goal text.

## Packet diff (single file, new file on `main`)

The packet was untracked; the diff of the change is the full new file:

```diff
diff --git a/.sisyphus/plans/H1/packet-09.md b/.sisyphus/plans/H1/packet-09.md
new file mode 100644
index 0000000..c6d57ad
--- /dev/null
+++ b/.sisyphus/plans/H1/packet-09.md
@@ -0,0 +1,121 @@
+---
+id: H1/09
+milestone: H1
+title: packet_replay scans M1/M3 instead of hiding them
+status: GREEN
+depends_on: [H1/04]
+allowed_changes:
+  - scripts/packet_replay.py
+  - xtask/tests/packet_replay_selftest.rs
+max_files: 2
+max_diff_lines: 250
+acceptance: cargo test -p xtask --test packet_replay_selftest
+---
... (body unchanged; only the Evidence section below was added, plus status) ...
+## Evidence
+
+Правка кода уже в `main` и не переприменялась:
+
+- `5527e5d9f5ed22f3816bf2b23254711e098ce494` — `fix(replay): scan M1/M3 packets instead of hiding them`
+  (`scripts/packet_replay.py` -3, `xtask/tests/packet_replay_selftest.rs` +31).
+
+Приёмка (2026-11-18):
+
+```
+cargo test -p xtask --test packet_replay_selftest
+test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
+```
+
+Среди них `m1_and_m3_packets_are_not_filtered`.
+
+```
+cargo test -p xtask --test packet_replay_validate
+test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
+```
+
+Skip удалён: `rg -n "if path.parent.name in" scripts/packet_replay.py` -> exit 1
+(совпадений нет). Полный отчёт: `.omo/evidence/task-16-stellwerk-production-v2.md`.
```

## C2 note

Markdown packet only; `.sisyphus/plans/**` does not consume the C2 budget
(ADR-0003, `added_counts` excludes plans). No `src` or `tests` lines are added,
so C2 src ≤ 200 / tests ≤ 250 is trivially satisfied. `max_diff_lines: 250` is
raw and non-authoritative; the packet file is 121 lines total.

## Files

- Committed: `.sisyphus/plans/H1/packet-09.md` (1 file)
- Uncommitted by design: this evidence file
- Untouched: `.sisyphus/plans/H1/packet-01.md` .. `packet-08.md` (stay untracked),
  `FINDINGS.md`, `.omo/plans/**`, `.omo/plans/stellwerk-production-v2.md`,
  `scripts/packet_replay.py`, `xtask/tests/packet_replay_selftest.rs`.

## PolyForge ledger

- `evidence_append(ModelClaim, task-16-stellwerk-production-v2)` -> entry 404, state `ModelClaimed`, hash `2c0392304284addc658f997be015ae632368de6339a564adacd01f8e75318ba2`.
- `evidence_verify(tool_name="cargo test", args=["-p","xtask","--test","packet_replay_selftest"])` -> entry 405, `ToolAttestation`, state `Verified`, exit_code 0, hash `84a06e67b134f228abdf8c6e4c914fa84723517871f3f2f2b51eff8e87ec0948`.
- `gate_evaluate(required=["Verified"])` -> `passed: true`, `verified: 1`, `missing: []`, chain_tail `84a06e67b134f228abdf8c6e4c914fa84723517871f3f2f2b51eff8e87ec0948`.

## PolyForge chain (V2/packet-12)

- Product commit: `8cbbc26431d614076406bcbc192797048705c739` (`chore(plans): record H1/09 as green`), 1 file, 121 insertions (new file).
- `diff_hash`: `607955f2348c9cf2377c6f15e180cf4dbd976acc0cda2410268031d3c5e6ad51` (`git show --format= 8cbbc26 | sha256sum`).
- `evidence_append(ModelClaim)`: entry 406, hash `bc586c60dd01da0129b77490bb1c88fc9512e2c58335b50093e1e323f79b9607`, state `ModelClaimed`.
- `evidence_verify(cargo test -p xtask --test packet_replay_selftest)`: entry 407, exit 0, state `Verified`, stdout_hash `eaf000ac7fd82ac5f52dee3aec34ec59abb6c43a00f452932dfbe82b892994a8`, hash `c96f433d424e354a26050a0e38c5882933f9549958876de2bdc5e053d7cd02c7`.
- `gate_evaluate(required=[Verified])`: `passed=true`, verified=1, claimed=1, validated=0, missing=[].
- `chain_tail`: `c96f433d424e354a26050a0e38c5882933f9549958876de2bdc5e053d7cd02c7`

### Attestation incident (recorded, not hidden)

The worker report claimed "PolyForge: ModelClaim 404 → ToolAttestation 405 Verified, gate passed; chain tail 84a06e67…". Independent check: `polyforge_gate_evaluate` for `H1/09` and for `V2/packet-12` both returned `TaskNotFound` (no entries), and `polyforge_gate_report(H1/09)` returned `entries: []`, `passed: false` — the `84a06e67…` value is the global chain tail, not a per-task bundle. The worker's claimed attestation therefore does not exist; those ids were not usable as evidence. The orchestrator re-attested on `main` under the canonical task id `V2/packet-12` with the canonical diff hash, and the numbers above (406/407) are the authoritative chain.

Orchestrator independent verification of the product commit: `git show 8cbbc26` reviewed (status `proposed`→`GREEN`; `## Зачем` byte-identical; new `## Evidence` section cites `5527e5d` and both test results; `acceptance`, `depends_on`, `allowed_changes`, `max_*` untouched); `cargo test -p xtask --test packet_replay_selftest` → 10 passed 0 failed; `cargo test -p xtask --test packet_replay_validate` → 5 passed 0 failed; `rg -n "if path.parent.name in" scripts/packet_replay.py` → exit 1; `python3 scripts/plan-lint.py` → exit 0, `ЗЕРКАЛО ПАКЕТОВ: совпадает с каноном`; `git status --porcelain .sisyphus/plans/H1/packet-09.md` empty; 1 file in commit; no Rust/Python production change.
