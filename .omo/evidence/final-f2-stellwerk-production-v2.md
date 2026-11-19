# F2 Code-Quality Review — stellwerk-production-v2

- Plan: `.omo/plans/stellwerk-production-v2.md` (F2, ~line 280)
- Repo: `/home/izyabretatel/gits/stellwerk`
- Branch: `main`
- HEAD: `5d7e50b` (`chore(evidence): attest V2/packet-14 after merge`)
- Timestamp (UTC): `2026-11-19T00:28:05Z`
- Reviewer mode: read-only audit; no source/test/plan/packet/ledger/boulder/notepad modifications.

## Command 1 — formatting

Command:

```
cargo fmt --all -- --check; echo "FMT_EXIT=$?"
```

Raw output:

```
FMT_EXIT=0
```

Result: **PASS** (exit 0, no diff emitted).

## Command 2 — clippy (fresh full-tree run)

The workspace default target dir already held up-to-date clippy artifacts
(incremental re-run finished in 0.29s, exit 0, 0 warnings). To avoid any
chance of cached diagnostics masking a V2 warning, clippy was re-run against a
**fresh** target dir (kept outside the repo so the git tree stays clean). `/tmp`
had 14G free; the fresh target lived under `/home/izyabretatel/.cache` and is
noted here for reproducibility.

Command:

```
export CARGO_TARGET_DIR=/home/izyabretatel/.cache/f2-clippy-target
OUT=/home/izyabretatel/.cache/f2-clippy-fresh.log
set -o pipefail
cargo clippy --workspace --all-targets -- -D warnings > "$OUT" 2>&1; RC=$?
echo "CLIPPY_FRESH_EXIT=$RC"
```

Raw output (tail):

```
   Compiling thiserror v2.0.20
   Compiling thiserror-impl v2.0.20
    Checking stellwerk-registry v0.0.1 (/home/izyabretatel/gits/stellwerk/crates/stellwerk-registry)
    Checking stellwerk-store v0.0.1 (/home/izyabretatel/gits/stellwerk/crates/stellwerk-store)
    Checking stellwerk-gateway v0.0.1 (/home/izyabretatel/gits/stellwerk/crates/stellwerk-gateway)
    Checking stellwerk-task v0.0.1 (/home/izyabretatel/gits/stellwerk/crates/stellwerk-task)
    Checking stellwerk-cli v0.0.1 (/home/izyabretatel/gits/stellwerk/crates/stellwerk-cli)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 29.60s
CLIPPY_FRESH_EXIT=0
```

Diagnostic scan of the full fresh log:

```
warnings=0
(grep 'error' -> only "Compiling thiserror v2.0.20" and "Compiling thiserror-impl v2.0.20"; no compile errors)
```

The earlier incremental run against the workspace target dir:

```
    Checking stellwerk-task v0.0.1 (/home/izyabretatel/gits/stellwerk/crates/stellwerk-task)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.29s
CLIPPY_EXIT=0
```

Result: **PASS** — exit 0 with `-D warnings`, zero warnings, zero errors across all
targets (lib, bins, tests).

## Command 3 — suppression scan

Run from repo root. Raw run (as specified):

```
rg -n '#\[allow|#\[ignore\]' crates/
```

Verbatim output:

```
crates/stellwerk-registry/src/cmd_update.rs:254:#[allow(clippy::too_many_arguments)]
```

Exit code: `0` — match count: **1**.

Filtered run (exclude build artifacts):

```
rg -n '#\[allow|#\[ignore\]' crates/ --glob '!**/target/**'
```

Verbatim output:

```
crates/stellwerk-registry/src/cmd_update.rs:254:#[allow(clippy::too_many_arguments)]
```

Exit code: `0` — match count: **1**.

The raw and filtered runs are identical: no match under any `target/` directory, so
build artifacts do not pollute the result. The single match is exactly the
pre-existing attribute named in the plan.

Result: **PASS** — the only match is `crates/stellwerk-registry/src/cmd_update.rs:254`.

## Pre-existence evidence (git blame / log)

```
$ git log -1 --format='%h %ad %s' --date=short -- crates/stellwerk-registry/src/cmd_update.rs
26caaed 2026-09-26 feat(registry): complete M0 — measure, calibration, update

$ git blame -L 254,254 -- crates/stellwerk-registry/src/cmd_update.rs
26caaed (Aleksei Schweigert 2026-09-26 23:59:29 +0200 254) #[allow(clippy::too_many_arguments)]

$ git merge-base --is-ancestor 26caaed HEAD && echo YES
YES: 26caaed is ancestor of HEAD
```

`crates/stellwerk-registry/src/cmd_update.rs:250-258` source context:

```
250:         }
251:     }
252: }
253:
254: #[allow(clippy::too_many_arguments)]
255: fn pipeline(
256:     root: &Path,
257:     name: &str,
258:     url: &str,
```

The attribute was introduced by `26caaed` on 2026-09-26 (M0), which is an ancestor
of HEAD. The V2 work landed 2026-11-18/19 (e.g. `096f3d0` 2026-11-18,
`d2d1ce6` 2026-11-19). The attribute therefore **predates V2** and is not a new
suppression introduced during this wave. It was **NOT removed** (per the plan).

## FINDING F-2026-11-10-01

- Kind: pre-existing lint suppression, out-of-scope for V2.
- Location: `crates/stellwerk-registry/src/cmd_update.rs:254`
- Attribute/lint: `#[allow(clippy::too_many_arguments)]`
- Introduced by: `26caaed` (2026-09-26, M0 — "feat(registry): complete M0").
- Status: **NOT removed** in V2. `crates/stellwerk-registry/src/cmd_update.rs` is
  not a V2 packet file; removing it is outside F2 scope and is deferred.

## Verdict

- `cargo fmt --all -- --check` → exit 0.
- `cargo clippy --workspace --all-targets -- -D warnings` → exit 0, 0 warnings, 0 errors (fresh full-tree run).
- `rg '#\[allow|#\[ignore\]' crates/` → exactly one match, `crates/stellwerk-registry/src/cmd_update.rs:254`, confirmed pre-existing via blame (`26caaed`, 2026-09-26) and left untouched.
- FINDING `F-2026-11-10-01` recorded; no suppression added or removed during V2.

F2 code-quality review: **PASS**.
