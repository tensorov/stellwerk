# task-12 — V2-08 / F-09: sync the .omo/plans mirror with canon

Packet: `.sisyphus/plans/V2/packet-08.md`
Canon: `.sisyphus/plans/` (read-only). Mirror: `.omo/plans/` (source of truth is canon).

## 1. C8 drift BEFORE (full list from `check_mirror_drift`, not the truncated `... ещё N` report)

The script truncates the printed list at 12 entries; the full list was obtained by
importing `scripts/plan-lint.py` at runtime (`m.check_mirror_drift('.')`) — no script edit.

```
TOTAL 16
C8 N2/packet-00i1.md :: зеркало разошлось с каноном: depends_on — канон `depends_on: [N2/00h3e, 00e, 00f]` против зеркала `depends_on: [N2/00h3e]`
C8 N2/packet-00j1.md :: зеркало разошлось с каноном: depends_on — канон `depends_on: [00h3c, 00h3d]` против зеркала `depends_on: []`
C8 N2/packet-00j2.md :: зеркало разошлось с каноном: depends_on — канон `depends_on: [00d, 00i1]` против зеркала `depends_on: []`
C8 N2/packet-00j3.md :: зеркало разошлось с каноном: depends_on — канон `depends_on: [N2/00j1, N2/00j2, 00j2]` против зеркала `depends_on: [N2/00j1, N2/00j2]`
C8 N2/packet-00j3a.md :: зеркало разошлось с каноном: depends_on — канон `depends_on: [N2/00j3, 00h2a]` против зеркала `depends_on: [N2/00j3]`
C8 N2/packet-00j4.md :: зеркало разошлось с каноном: depends_on — канон `depends_on: [N2/00j3a, 00g3b]` против зеркала `depends_on: [N2/00j3a]`
C8 N2/packet-00j5.md :: зеркало разошлось с каноном: depends_on — канон `depends_on: [N2/00j4, 00j3, 00j4]` против зеркала `depends_on: [N2/00j4]`
C8 N2/packet-00j6.md :: зеркало разошлось с каноном: depends_on — канон `depends_on: [N2/00j5, 00f, 00i1, 00j1]` против зеркала `depends_on: [N2/00j5]`
C8 N2/packet-00j7.md :: зеркало разошлось с каноном: depends_on — канон `depends_on: [N2/00j6, 00h1, 00j3, 00j5]` против зеркала `depends_on: [N2/00j6]`
C8 N2/packet-00j7a.md :: зеркало разошлось с каноном: depends_on — канон `depends_on: [N2/00j7, 00j5]` против зеркала `depends_on: [N2/00j7]`
C8 V1/packet-04.md :: зеркало разошлось с каноном: depends_on — канон `depends_on: [r01, r04, a01, a01b, a02, a03, v02, o02]` против зеркала `depends_on: [r01, r04, a01, a02, a03, v02, o02]`
C8 V1/packet-a02.md :: зеркало разошлось с каноном: depends_on — канон `depends_on: [a01, a01b, r03]` против зеркала `depends_on: [a01, r03]`
C8 V1/packet-v01.md :: зеркало разошлось с каноном: depends_on — канон `depends_on: [a02b, r04]` против зеркала `depends_on: [a02, r04]`
C8 V1/packet-v02.md :: зеркало разошлось с каноном: files_allowed — канон `files_allowed: [crates/stellwerk-task/src/m5/model_transport.rs, crate…` против зеркала `files_allowed: [crates/stellwerk-cli/tests/vertical_v2.rs]`
C8 V1/packet-v02.md :: зеркало разошлось с каноном: acceptance — канон `acceptance: cargo test -p stellwerk-task --test model_transport_wire` против зеркала `acceptance: cargo test -p stellwerk-cli --test vertical_v2`
C8 V1/packet-v03b.md :: зеркало разошлось с каноном: acceptance — канон `acceptance: cargo test -p stellwerk-task -p stellwerk-cli --test evide…` против зеркала `acceptance: cargo test -p stellwerk-task --test evidence && cargo test…`
```

Distinct diverging packet files: 10 under `N2/`, 5 under `V1/` (`v02` produced 2 field
entries). `V2/` was entirely absent from the mirror, so C8 was blind to its 17 canon
packets (C8 only walks the mirror).

## 2. Operations performed

No mirror packet lacked a canonical counterpart (checked with
`find .omo/plans -name 'packet-*.md'`), so **no `mv` to `_archive/` was needed** and no
archive collision was possible.

```
mkdir -p .omo/plans/V2
cp -a .sisyphus/plans/V2/. .omo/plans/V2/
cp -a .sisyphus/plans/V1/. .omo/plans/V1/
cp -a .sisyphus/plans/N2/. .omo/plans/N2/
```

Whole V1/N2/V2 trees were copied (not just the C8-named files), because `diff -rq`
acceptance requires the trees to be identical and C8 is blind to canon-only files.
Nothing under `.sisyphus/` was modified.

## 3. C8 AFTER

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
```

`ЗЕРКАЛО ПАКЕТОВ: совпадает с каноном`; the `ПЕРЕСЕЧЕНИЯ МЕЖДУ MILESTONE` and
`ТАБЛИЦА risk_class` sections are still printed.

## 4. `diff -rq` exit codes (0 = identical)

```
diff -rq .sisyphus/plans/V2 .omo/plans/V2   -> exit 0
diff -rq .sisyphus/plans/V1 .omo/plans/V1   -> exit 0
diff -rq .sisyphus/plans/N2 .omo/plans/N2   -> exit 0
python3 scripts/plan-lint.py .sisyphus/plans/V1 -> exit 0
```

`.omo/plans/V2/` exists with all 17 canonical packets (`ls .omo/plans/V2 | wc -l` = 17).

## 5. Staged names before the commit (`git add -A .omo/plans`)

```
.omo/plans/H1/FINDINGS.md
.omo/plans/H1/packet-01.md
.omo/plans/H1/packet-02.md
.omo/plans/H1/packet-03.md
.omo/plans/H1/packet-04.md
.omo/plans/H1/packet-05.md
.omo/plans/H1/packet-06.md
.omo/plans/H1/packet-07.md
.omo/plans/H1/packet-08.md
.omo/plans/H1/packet-09.md
.omo/plans/M1/packet-05.md
.omo/plans/M1/packet-06.md
.omo/plans/M3/REVIEW.md
.omo/plans/N1.md
.omo/plans/N1/REVIEW.md
.omo/plans/N1/index.md
.omo/plans/N1/packet-01.md
.omo/plans/N1/packet-02.md
.omo/plans/N1/packet-03.md
.omo/plans/N1/packet-04.md
.omo/plans/N1/packet-05.md
.omo/plans/N1/packet-06.md
.omo/plans/N1/packet-07.md
.omo/plans/N1/packet-08.md
.omo/plans/N1/packet-09.md
.omo/plans/N1/packet-10.md
.omo/plans/N2/packet-00b.md
.omo/plans/N2/packet-00c.md
.omo/plans/N2/packet-00d.md
.omo/plans/N2/packet-00e.md
.omo/plans/N2/packet-00f.md
.omo/plans/N2/packet-00g1.md
.omo/plans/N2/packet-00g2.md
.omo/plans/N2/packet-00g3.md
.omo/plans/N2/packet-00g3b.md
.omo/plans/N2/packet-00h1.md
.omo/plans/N2/packet-00h2a.md
.omo/plans/N2/packet-00h2b.md
.omo/plans/N2/packet-00h3a.md
.omo/plans/N2/packet-00h3b.md
.omo/plans/N2/packet-00h3c.md
.omo/plans/N2/packet-00h3d.md
.omo/plans/N2/packet-00h3e.md
.omo/plans/N2/packet-00i1.md
.omo/plans/N2/packet-00j1.md
.omo/plans/N2/packet-00j2.md
.omo/plans/N2/packet-00j3.md
.omo/plans/N2/packet-00j3a.md
.omo/plans/N2/packet-00j4.md
.omo/plans/N2/packet-00j5.md
.omo/plans/N2/packet-00j6.md
.omo/plans/N2/packet-00j7.md
.omo/plans/N2/packet-00j7a.md
.omo/plans/RIEGEL/index.md
.omo/plans/TUI/packet-00.md
.omo/plans/TUI/packet-01.md
.omo/plans/TUI/packet-02.md
.omo/plans/TUI/packet-03.md
.omo/plans/TUI/packet-04.md
.omo/plans/V1/packet-00.md
.omo/plans/V1/packet-01.md
.omo/plans/V1/packet-02.md
.omo/plans/V1/packet-03.md
.omo/plans/V1/packet-04.md
.omo/plans/V1/packet-a01.md
.omo/plans/V1/packet-a01b.md
.omo/plans/V1/packet-a02.md
.omo/plans/V1/packet-a02b.md
.omo/plans/V1/packet-a03.md
.omo/plans/V1/packet-c01.md
.omo/plans/V1/packet-c02.md
.omo/plans/V1/packet-i01.md
.omo/plans/V1/packet-i02.md
.omo/plans/V1/packet-l01.md
.omo/plans/V1/packet-o01.md
.omo/plans/V1/packet-o02.md
.omo/plans/V1/packet-o03.md
.omo/plans/V1/packet-o04.md
.omo/plans/V1/packet-q01.md
.omo/plans/V1/packet-q02.md
.omo/plans/V1/packet-r01.md
.omo/plans/V1/packet-r02.md
.omo/plans/V1/packet-r03.md
.omo/plans/V1/packet-r04.md
.omo/plans/V1/packet-s01.md
.omo/plans/V1/packet-s02.md
.omo/plans/V1/packet-s03.md
.omo/plans/V1/packet-v01.md
.omo/plans/V1/packet-v02.md
.omo/plans/V1/packet-v02b.md
.omo/plans/V1/packet-v03.md
.omo/plans/V1/packet-v03b.md
.omo/plans/V1/packet-v04.md
.omo/plans/V2/packet-01.md
.omo/plans/V2/packet-02.md
.omo/plans/V2/packet-03.md
.omo/plans/V2/packet-04.md
.omo/plans/V2/packet-05a.md
.omo/plans/V2/packet-05b.md
.omo/plans/V2/packet-06.md
.omo/plans/V2/packet-07a.md
.omo/plans/V2/packet-07b.md
.omo/plans/V2/packet-07c.md
.omo/plans/V2/packet-08.md
.omo/plans/V2/packet-09.md
.omo/plans/V2/packet-10.md
.omo/plans/V2/packet-11.md
.omo/plans/V2/packet-12.md
.omo/plans/V2/packet-13.md
.omo/plans/V2/packet-14.md
.omo/plans/_archive/packet-14.md
.omo/plans/_archive/packet-15.md
.omo/plans/_archive/packet-16.md
.omo/plans/_archive/packet-17.md
.omo/plans/_archive/packet-18.md
.omo/plans/m6-async-diffy-gix.md
.omo/plans/m6b-firecracker-qwen.md
.omo/plans/omo-model-swap.md
.omo/plans/pi-tools-channel.md
.omo/plans/polyforge-trust-closure.md
.omo/plans/repo-hygiene.md
.omo/plans/stellwerk-attested-autonomy.md
.omo/plans/stellwerk-production-v1.md
.omo/plans/stellwerk-production-v2.md
.omo/plans/tui.md
```

(125 files, all under `.omo/plans/`. `.omo/omo.jsonc`, `opencode.json`,
`tests/test_omo_config.py` were left unstaged.)

## 6. After the commit

```
commit: [main 6799489] chore(plans): sync mirror with canon
git rev-parse HEAD = 679948990c30d4596dc2ab99b312f62118e7d6bd
git ls-files .omo/plans | wc -l = 126
git ls-files .omo/plans | grep 'V2/' | wc -l = 17
git status --porcelain .omo/plans -> (empty)
```

`.omo/plans/.gitkeep` remains tracked; service files (`stellwerk-production-v2.md`,
`tui.md`, `N1.md`, `polyforge-trust-closure.md`, `repo-hygiene.md`, `m6-*.md`,
`omo-model-swap.md`, `pi-tools-channel.md`, `RIEGEL/`) were preserved, not archived.

## 7. Out-of-scope finding (not fixed; see learnings)

Full-tree comparison also shows silent drift in `N1/` (bold `**depends_on:**` lines, which
C8 does not parse) and `TUI/` (body/SSH-host changes, not one of the four C8 fields).
They are not part of this packet's V1/N2/V2 acceptance and were left untouched.

## 8. PolyForge attestation (orchestrator, on main)

Acceptance command is `python3 scripts/plan-lint.py`, which is outside the PolyForge
allowlist (xtask/src/main.rs header: the python linter is not attestable directly).
Attestation therefore used the workspace guard that executes the same script:

```
cargo test -p stellwerk-cli --test plan_lint
```

`plan_lint_is_green` runs `python3 scripts/plan-lint.py` over the whole canon from the
repo root and fails on any `НАРУШЕНИЙ` in stdout/stderr, so a passing run proves the C8
mirror line and the absence of violations; it also asserts cross-milestone order is
declared everywhere.

- `evidence_append(ModelClaim, task_id=V2/packet-08)` -> seq 397, hash
  `56e91207b3f806a59035c21b20a778d630a655ace965ada98ee3c2ef3eb4a75e`,
  commit `679948990c30d4596dc2ab99b312f62118e7d6bd`, diff_hash
  `cb1f19ab6d74eed10435b19315f46166afd3af46b6a0d8622fb644adae7f16a3`.
- `evidence_verify(cargo test -p stellwerk-cli --test plan_lint)` -> seq 398
  `Verified`, exit 0, stdout_hash
  `6b3ce812666064e92d38b198371368e6c6791ec31bff4ee95470d0754ed39362`,
  `commit_sha` matches, `diff_hash` matches.
- `gate_evaluate(required=["Verified"])` -> passed, claimed 1 / verified 1 /
  validated 0, missing `[]`, chain tail
  `31d19800d23663c28464341f80018421ecb739fc7919acf4c3c87a62ab64d0cf`.

Orchestrator re-verification before ticking the checkbox (independent of worker
report): `git show --numstat 6799489` -> 125 files, all `.omo/plans/**`;
`python3 scripts/plan-lint.py` -> `ЗЕРКАЛО ПАКЕТОВ: совпадает с каноном`, exit 0;
`diff -rq` V2/V1/N2 exit 0 each; `git status --porcelain .omo/plans` empty.
