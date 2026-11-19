# F4 — Scope fidelity audit (stellwerk-production-v2)

- Timestamp (UTC): 2026-11-19T00:28:32Z
- Repo: `/home/izyabretatel/gits/stellwerk`, branch `main`
- Audited range: `0b2b738..5d7e50b`
  - base = `0b2b738e2856b8ff447210b0f9f03fdf96e33895` `chore(plans): commit V1 packet updates` (V1 base)
  - head = `5d7e50bf2e4f64cd09133d088f009eaa26babae9` `chore(evidence): attest V2/packet-14 after merge`
- Plan item: `F4. Scope fidelity` — "diff against the V1 base; confirm no new dependency, no `contract.rs` edit outside V2-07a, no `modules.lock`/workflow/root `Cargo.toml` edit, no `CandidateFile::Removed`, and no budget change without a reproduced failure."
- Commits in range: **32** (`git log --oneline 0b2b738..5d7e50b | wc -l`).

This is a read-only audit. No source, test, plan, packet, ledger, boulder, or notepad file was modified; no commit was made; PolyForge was not called.

---

## 0. Aggregate diff

Command:

```
git diff --stat 0b2b738..5d7e50b
```

Tail: `180 files changed, 15256 insertions(+), 87 deletions(-)` (full diffstat captured to `/tmp/f4-diffstat.txt` during the audit; every path is reproduced in the grouped list below).

Changed paths grouped by category (180 total = 21 `crates/` + 2 `docs/adr/` + 19 `.sisyphus/plans/` + 125 `.omo/plans/` + 13 `.omo/evidence/`):

### src (10) — `crates/**/src/`

```
crates/stellwerk-cli/src/build_runner.rs
crates/stellwerk-cli/src/host_model.rs
crates/stellwerk-cli/src/lib.rs
crates/stellwerk-cli/src/main.rs
crates/stellwerk-cli/src/plan.rs
crates/stellwerk-core/src/contract.rs
crates/stellwerk-task/src/evidence.rs
crates/stellwerk-task/src/lifecycle.rs
crates/stellwerk-task/src/runner.rs
crates/stellwerk-task/src/taskfile.rs
```

### tests (10) — `crates/**/tests/`

```
crates/stellwerk-cli/tests/candidate.rs
crates/stellwerk-cli/tests/plan.rs
crates/stellwerk-cli/tests/run.rs
crates/stellwerk-task/tests/evidence.rs
crates/stellwerk-task/tests/human_validation.rs
crates/stellwerk-task/tests/ledger_sqlite.rs
crates/stellwerk-task/tests/lifecycle.rs
crates/stellwerk-task/tests/refutation.rs
crates/stellwerk-task/tests/runner.rs
crates/stellwerk-task/tests/taskfile.rs
```

### other crate file (1)

```
crates/stellwerk-cli/examples/n1-smoke.toml
```

### docs / ADR (2)

```
docs/adr/0015-execution-spec-not-task-spec.md
docs/adr/0024-refutation-evidence-kind.md
```

### plans — canon `.sisyphus/plans/` (19)

```
.sisyphus/plans/H1/packet-09.md
.sisyphus/plans/V1/packet-01.md
.sisyphus/plans/V2/packet-01.md
.sisyphus/plans/V2/packet-02.md
.sisyphus/plans/V2/packet-03.md
.sisyphus/plans/V2/packet-04.md
.sisyphus/plans/V2/packet-05a.md
.sisyphus/plans/V2/packet-05b.md
.sisyphus/plans/V2/packet-06.md
.sisyphus/plans/V2/packet-07a.md
.sisyphus/plans/V2/packet-07b.md
.sisyphus/plans/V2/packet-07c.md
.sisyphus/plans/V2/packet-08.md
.sisyphus/plans/V2/packet-09.md
.sisyphus/plans/V2/packet-10.md
.sisyphus/plans/V2/packet-11.md
.sisyphus/plans/V2/packet-12.md
.sisyphus/plans/V2/packet-13.md
.sisyphus/plans/V2/packet-14.md
```

### plans — OMO mirror / drafts `.omo/plans/` (125)

`.omo/plans/` — 125 files (`.omo/plans/V2/packet-*.md`, other milestones `H1/ M1/ M3/ N1/ N2/ RIEGEL/ TUI/ V1/`, `_archive/`, and draft plans such as `m6-*.md`, `tui.md`, `repo-hygiene.md`, etc.). This is orchestrator/plan bookkeeping, not product code; listed in aggregate because it is all under the allowed `.omo/plans/` prefix.

### evidence `.omo/evidence/` (13)

```
.omo/evidence/task-6-stellwerk-production-v2.md
.omo/evidence/task-7-stellwerk-production-v2.md
.omo/evidence/task-8-stellwerk-production-v2.md
.omo/evidence/task-9-stellwerk-production-v2.md
.omo/evidence/task-10-stellwerk-production-v2.md
.omo/evidence/task-11-stellwerk-production-v2.md
.omo/evidence/task-12-stellwerk-production-v2.md
.omo/evidence/task-13-stellwerk-production-v2.md
.omo/evidence/task-14-stellwerk-production-v2.md
.omo/evidence/task-15-stellwerk-production-v2.md
.omo/evidence/task-16-stellwerk-production-v2.md
.omo/evidence/task-17-stellwerk-production-v2.md
.omo/evidence/task-18-stellwerk-production-v2.md
```

### Path-fence check

Command:

```
git diff --name-only 0b2b738..5d7e50b \
  | rg -v '^(crates/|docs/adr/|\.sisyphus/plans/|\.omo/plans/|\.omo/evidence/|scripts/|xtask/)'
```

Output: `NONE` — every changed path lies inside the allowed set (`crates/`, `docs/adr/`, `.sisyphus/plans/`, `.omo/plans/`, `.omo/evidence/`). `scripts/` and `xtask/` changed 0 files.

---

## 1. Dependency check — PASS

Command (verbatim):

```
git diff 0b2b738..5d7e50b -- Cargo.toml Cargo.lock crates/*/Cargo.toml
```

Output: **empty** (exit 0, zero hunks).

Glob form:

```
git diff 0b2b738..5d7e50b -- '**/Cargo.toml' 'Cargo.lock' '**/Cargo.lock'
```

Output: **empty**.

Cross-check on path names:

```
git diff --name-only 0b2b738..5d7e50b | rg -n 'Cargo\.(toml|lock)$'
```

Output: `NONE`.

Conclusion: no `Cargo.toml` and no `Cargo.lock` was added, removed, or edited anywhere in the range; therefore no new dependency, no version bump, no workspace-member change.

---

## 2. `contract.rs` — PASS (exactly V2-07a)

Command:

```
git log --oneline 0b2b738..5d7e50b -- crates/stellwerk-core/src/contract.rs
```

Output:

```
096f3d0 feat(contract): add Refuted evidence kind
```

Exactly one commit. Its shape:

```
git show --numstat --format='%H %s' 096f3d0 -- crates/stellwerk-core/src/contract.rs
```

```
096f3d0fa7484a58cf74281c9f9102e55a8c2e2a feat(contract): add Refuted evidence kind
30	0	crates/stellwerk-core/src/contract.rs
```

`096f3d0` is the V2-07a product commit ("add Refuted evidence kind", paired with evidence commit `d89436a`). No other commit in `0b2b738..5d7e50b` touched `crates/stellwerk-core/src/contract.rs`.

---

## 3. Protected paths — PASS (each reported separately)

### 3a. `modules.lock`

```
git diff --name-only 0b2b738..5d7e50b -- modules.lock
```
Output: **empty**.

### 3b. `.github/workflows/`

```
git diff --name-only 0b2b738..5d7e50b -- .github/workflows
```
Output: **empty**.

### 3c. root `Cargo.toml`

```
git diff --name-only 0b2b738..5d7e50b -- Cargo.toml
```
Output: **empty** (also covered by §1).

### 3d. `crates/*/Cargo.toml`

```
git diff --name-only 0b2b738..5d7e50b -- 'crates/*/Cargo.toml'
```
Output: **empty** (also covered by §1).

All four protected targets are untouched.

---

## 4. `CandidateFile::Removed` / `CandidateVerdict::Removed` — PASS (absent)

`CandidateFile` is a struct, so it cannot carry a `Removed` variant:

```rust
// crates/stellwerk-cli/src/build_runner.rs:128-132
pub struct CandidateFile {
    pub path: String,
    pub digest: String,
}
```

The verdict enum has exactly two arms, neither `Removed`:

```
$ rg -n "enum CandidateVerdict" -A 10 crates/stellwerk-cli/src/build_runner.rs
213:pub enum CandidateVerdict {
214-    Clean,
215-    Dirty(DirtyWorkspace),
216-}
```

Searches:

```
$ rg -n "Removed" crates/stellwerk-cli/src/
NONE

$ rg -n "Removed" crates/stellwerk-cli/tests/candidate.rs
NONE
```

The string `Removed` does not occur at all in `crates/stellwerk-cli/src/` or in `tests/candidate.rs`; the candidate verdict did not gain a `Removed` arm.

---

## 5. Budgets — PASS (unchanged from V1 base)

Current values (read at HEAD `5d7e50b`):

```
$ rg -n "BUILD_WINDOW|BUILD_BUDGET|TEST_BUDGET" crates/stellwerk-cli/src/build_runner.rs
592:pub const BUILD_WINDOW: Duration = Duration::from_secs(60);
593:pub const BUILD_BUDGET: Duration = Duration::from_secs(30);
...

$ rg -n "TEST_BUDGET_MS|BUILD_WINDOW|BUILD_BUDGET" crates/stellwerk-cli/tests/audit_live.rs
17:const TEST_BUDGET_MS: u64 = 50;
```

Base values (at `0b2b738`):

```
$ git show 0b2b738:crates/stellwerk-cli/src/build_runner.rs | rg -n "BUILD_WINDOW|BUILD_BUDGET"
584:pub const BUILD_WINDOW: Duration = Duration::from_secs(60);
585:pub const BUILD_BUDGET: Duration = Duration::from_secs(30);

$ git show 0b2b738:crates/stellwerk-cli/tests/audit_live.rs | rg -n "TEST_BUDGET_MS"
17:const TEST_BUDGET_MS: u64 = 50;
```

Diff over the budget constants:

```
$ git diff 0b2b738..5d7e50b -- crates/stellwerk-cli/src/build_runner.rs | rg -n "BUILD_WINDOW|BUILD_BUDGET"
NO BUDGET LINES CHANGED

$ git diff 0b2b738..5d7e50b -- crates/stellwerk-cli/tests/audit_live.rs | rg -n "TEST_BUDGET_MS"
NO BUDGET LINES CHANGED
```

Whole-range scan for any budget/window delta:

```
$ git diff 0b2b738..5d7e50b | rg -n "^[+-].*(BUILD_WINDOW|BUILD_BUDGET|TEST_BUDGET|_BUDGET|_WINDOW)"
# only matches inside added .omo/evidence prose (task-17 file), no code constant line
```

Note: `build_runner.rs` line numbers shifted `584/585 -> 592/593` only because V2-14 added an import and symlink-handling logic *above* the constants (see §6); the constant values themselves are byte-identical (`60s` / `30s`), and `audit_live.rs` was not modified at all in the range (`git diff --name-only 0b2b738..5d7e50b -- crates/stellwerk-cli/tests/audit_live.rs` = empty).

Decision context: `bdd546a test(cli): re-measure audit and real-Pi budgets` is **evidence-only** (`git show --stat bdd546a` = 1 file, 173 insertions, `.omo/evidence/task-17-stellwerk-production-v2.md`, no code). That evidence explicitly records "No budget change needed" for all three constants and classifies the observed failure as a test-harness invocation-path defect, not a timing defect (`.omo/evidence/task-17-stellwerk-production-v2.md:131-151`). Since the constants did not move and no budget change exists, the "budget change without a reproduced failure" condition is not triggered.

---

## 6. Supplementary scope observations (informational, not gating)

- `crates/stellwerk-cli/src/build_runner.rs` (+13/-5) is the V2-14 commit `d2d1ce6 fix(cli): treat symlink target changes as dirty`. It adds `std::os::unix::ffi::OsStrExt` and records a symlink's *target text* digest instead of ignoring symlinks; a changed/removed link target now becomes `Dirty`. This strengthens AGENTS.md law 6 (missing data is a distinct state, never silently `Clean`) and does not touch budgets, manifests, or the candidate verdict shape.
- `#[allow(...)]` / `#[ignore]` added anywhere under `crates/**` in the range: `NONE`.
- Working-tree files modified but **outside** the audited commit range (not part of `0b2b738..5d7e50b`; reported for completeness): `.omo/omo.jsonc`, `.omo/plans/stellwerk-production-v2.md`, `opencode.json`, `tests/test_omo_config.py`. Per the plan's F1 baseline these (plus untracked `.playwright-mcp/`, `mutants.out*`) are accepted pre-existing/orchestrator state and are not scope violations.

---

## Per-item results

| # | Item | Command(s) | Result |
|---|------|-----------|--------|
| 0 | Aggregate diff + path fence | `git diff --stat 0b2b738..5d7e50b`; `--name-only` fence `rg -v` | **PASS** — 180 files, all inside `crates/`, `docs/adr/`, `.sisyphus/plans/`, `.omo/plans/`, `.omo/evidence/`; 0 outside |
| 1 | No new dependency / manifest change | `git diff … -- Cargo.toml Cargo.lock crates/*/Cargo.toml`; `'**/Cargo.toml' 'Cargo.lock'` | **PASS** — empty |
| 2 | `contract.rs` only via V2-07a | `git log … -- contract.rs`; `git show --numstat 096f3d0` | **PASS** — single commit `096f3d0` (+30/-0) |
| 3a | `modules.lock` untouched | `git diff --name-only … -- modules.lock` | **PASS** — empty |
| 3b | `.github/workflows/` untouched | `git diff --name-only … -- .github/workflows` | **PASS** — empty |
| 3c | root `Cargo.toml` untouched | `git diff --name-only … -- Cargo.toml` | **PASS** — empty |
| 3d | `crates/*/Cargo.toml` untouched | `git diff --name-only … -- 'crates/*/Cargo.toml'` | **PASS** — empty |
| 4 | No `CandidateFile::Removed` / `CandidateVerdict::Removed` | `rg -n "enum CandidateVerdict" -A 10 …`; `rg -n "Removed" …` | **PASS** — `CandidateVerdict { Clean, Dirty(..) }`; `CandidateFile` is a struct; no `Removed` in `cli/src` or `tests/candidate.rs` |
| 5 | No budget change without reproduction | current vs base constant read; `git diff …` budget grep; task-17 evidence | **PASS** — `BUILD_WINDOW=60s`, `BUILD_BUDGET=30s`, `TEST_BUDGET_MS=50` unchanged from `0b2b738`; no change made; todo 17 recorded "no change needed" |
| 6 | Code-shape guard (`#[allow]`/`#[ignore]`) | `git diff … -- 'crates/**' \| rg '^\+.*(#\[allow\|#\[ignore)'` | **PASS** — none added |

---

## Verdict

All six F4 scope-fidelity conditions hold against the V1 base `0b2b738` at HEAD `5d7e50b`:

1. No new dependency — no `Cargo.toml`/`Cargo.lock` hunk anywhere.
2. `crates/stellwerk-core/src/contract.rs` was touched by exactly one commit, `096f3d0` (V2-07a, "add Refuted evidence kind", +30/-0); nothing else.
3. `modules.lock`, `.github/workflows/`, root `Cargo.toml`, and all `crates/*/Cargo.toml` are untouched.
4. `CandidateVerdict` has no `Removed` arm and `CandidateFile` is a struct; `Removed` appears nowhere in the candidate code or its tests.
5. The three budget constants are byte-identical to the V1 base (`60s` / `30s` / `50`); the only budget-related commit is evidence-only and explicitly decided "no change needed".
6. Every changed path is inside the allowed prefixes; no `#[allow]`/`#[ignore]` was added.

VERDICT: APPROVE
